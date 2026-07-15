#!/usr/bin/env python3
"""
Runner que os atalhos Steam (um por jogo, ver src/gameShortcuts.ts) executam.

Por que existe: o Gamescope (compositor do Modo Jogo) so foca/mostra janelas
lancadas atraves do mecanismo real da Steam - um subprocess solto (o que a
Fase 1 fazia) abre em fullscreen mas fica "escondido" atras da UI, sem foco
nenhum (confirmado rodando no device). A solucao (igual o MoonDeck faz) e'
registrar este script como atalho non-Steam; a Steam entao o executa de
verdade (Gamescope trata como jogo, foca normalmente).

MUDANCA IMPORTANTE (atalhos por jogo, visiveis na biblioteca): antes, este
script so' dava exec no Moonlight - quem configurava o Apollo (login,
prep-cmd, cmd) era sempre o JS do plugin, chamado ANTES do lancamento via
stream_game(). Isso funcionava porque era sempre o NOSSO botao que disparava
o clique. Agora que os atalhos sao itens normais da biblioteca (usuario
clica "Jogar" nativo da Steam, sem passar pelo nosso codigo), nosso JS
NUNCA roda antes do lancamento - entao este script precisa se
auto-configurar: ler config/perfis do disco, detectar contexto, falar com
o Apollo, e SO' DEPOIS dar exec no Moonlight. Por isso importa
moonprofile_core (mesma logica que main.py usa) em vez de so' receber
variaveis de ambiente ja prontas.

Como recebe o parametro que importa: MOONPROFILE_HOST_APP_ID e' fixado nas
Launch Options do atalho UMA vez, na criacao (ver ensureGameShortcut em
src/gameShortcuts.ts) - e' o AppID real do jogo no Steam do HOST. Tudo mais
(qual perfil usar, config do Apollo) e' resolvido aqui, na hora do
lancamento, lendo os mesmos arquivos que o main.py le.

MUDANCA IMPORTANTE #2 (Apollo sem prep-cmd, Runner obrigatorio): o Apollo
NAO liga/desliga mais a tela do host sozinho - isso e' 100% responsabilidade
do MoonProfile Runner (Rust, ver moon_profile_runner/), tanto no lancamento
(register_with_runner manda os display_commands, que o Runner roda ANTES
de responder) quanto no fechamento (restore_commands, autonomo ou manual).
O Apollo fica so' com o "cmd" (conectar + rodar o jogo) - mais simples,
"plug and play", e da' ao Deck controle total sobre o ciclo de vida da
sessao. Por isso o Runner deixou de ser opcional: sem ele, a tela
simplesmente nao troca (ver main(), que aborta o lancamento se
register_with_runner falhar, igual ja fazia se configure_apollo falhasse).
"""
import os
import sys
import json
import urllib.error
import urllib.request

# runner.py fica em <PLUGIN_DIR>/runner/runner.py - py_modules e' irmao de
# runner/. Nao roda via Decky Loader (Steam executa direto), entao
# DECKY_PLUGIN_DIR/py_modules nao esta' no sys.path por padrao como
# aconteceria pra main.py (ver sandboxed_plugin.py do decky-loader) -
# precisa inserir manualmente.
_PLUGIN_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(_PLUGIN_DIR, "py_modules"))

from moonprofile_core import (  # noqa: E402 (import depois do sys.path.insert e' intencional)
    RUNNER_PORT,
    ApolloClient,
    CODEC_FLAGS,
    build_display_commands,
    build_restore_commands,
    classify_apollo_error,
    detect_context,
)

APP_NAME = "SteamGame"


def _decky_home() -> str:
    # <PLUGIN_DIR> = <DECKY_HOME>/plugins/moonprofile - mesma convencao que
    # o proprio decky-loader usa pra plugins/settings/data (todos irmaos
    # sob DECKY_HOME), so' que aqui descoberta em runtime em vez de vir
    # das variaveis de ambiente do loader (que nao existem pra este
    # processo).
    return os.path.dirname(os.path.dirname(_PLUGIN_DIR))


def _settings_dir() -> str:
    return os.path.join(_decky_home(), "settings", "moonprofile")


def _runtime_dir() -> str:
    return os.path.join(_decky_home(), "data", "moonprofile")


def _load_json(path: str) -> dict | list:
    with open(path) as f:
        return json.load(f)


def _pick_profile(profiles: list, context: str) -> dict | None:
    return next((p for p in profiles if p.get("trigger") == context), None)


def configure_apollo(host_app_id: str) -> dict:
    """
    Replica a parte de stream_game() do main.py que fala com o Apollo -
    login, salva o app "SteamGame" com o AppID deste jogo. O Apollo NAO
    recebe mais prep-cmd nenhum (nem do, nem undo) - quem liga/desliga a
    tela agora e' sempre o Runner (Rust), tanto no lancamento quanto no
    fechamento (ver register_with_runner/build_display_commands). Isso
    deixa o Apollo mais simples ("plug and play" - so' precisa saber
    conectar e rodar o cmd) e da' ao Deck controle total sobre o ciclo de
    vida da sessao. Levanta excecao se algo falhar (chamador decide o que
    fazer).
    """
    config = _load_json(os.path.join(_settings_dir(), "config.json"))
    profiles = _load_json(os.path.join(_settings_dir(), "profiles.json"))

    context = detect_context()
    profile = _pick_profile(profiles, context)
    if profile is None:
        raise RuntimeError(f"Nenhum perfil configurado pro contexto '{context}'")

    client = ApolloClient(config["host"], config["username"], config["password"])
    client.login()
    uuid = client.find_app_uuid(APP_NAME)
    client.save_app({
        "name": APP_NAME,
        "cmd": f"steam steam://rungameid/{host_app_id}",
        "uuid": uuid,
        "auto-detach": True,
        "wait-all": False,
        "exit-timeout": 5,
        "exclude-global-prep-cmd": False,
        "elevated": False,
        "prep-cmd": [],
        "output": f"/tmp/apollo-steamgame-{host_app_id}.log",
    })

    return {"config": config, "profile": profile}


def register_with_runner(config: dict, host_app_id: str, profile: dict) -> None:
    """
    Registra a sessao no MoonProfile Runner (daemon no host) - app_id +
    credenciais do Apollo EM MEMORIA (nunca gravadas em disco no host, ver
    session.rs), mais os comandos de LIGAR a tela (build_display_commands)
    e de RESTAURAR (build_restore_commands). O Runner roda os comandos de
    ligar a tela AGORA MESMO, de forma sincrona (essa chamada so' retorna
    depois disso) - e' por isso que precisa acontecer ANTES do exec no
    Moonlight, senao o stream comecaria antes da tela estar no estado
    certo.

    O Runner deixou de ser OPCIONAL por causa disso: como o Apollo nao
    tem mais prep-cmd nenhum, sem o Runner a tela simplesmente nao troca
    - por isso essa funcao levanta excecao em vez de so' logar e seguir
    (ver main() abaixo, que aborta o lancamento se isso falhar, do mesmo
    jeito que ja aborta se configure_apollo falhar).
    """
    body = json.dumps({
        "app_id": host_app_id,
        "username": config["username"],
        "password": config["password"],
        "display_commands": build_display_commands(profile["host"]),
        "restore_commands": build_restore_commands(profile["host"]),
    }).encode()
    req = urllib.request.Request(
        f"http://{config['host']}:{config.get('runner_port', RUNNER_PORT)}/session/register",
        data=body,
        method="POST",
    )
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=30):  # 30s: da tempo dos display_commands rodarem no Runner
        pass
    print(f"Sessao registrada no Runner ({config['host']}:{config.get('runner_port', RUNNER_PORT)}) pro app_id={host_app_id}", file=sys.stderr)


def main() -> None:
    host_app_id = os.environ.get("MOONPROFILE_HOST_APP_ID")
    if not host_app_id:
        print("MOONPROFILE_HOST_APP_ID nao definido - abortando", file=sys.stderr)
        sys.exit(1)

    log_path = os.path.join(_runtime_dir(), "moonlight.log")
    os.makedirs(_runtime_dir(), exist_ok=True)
    # Redireciona stdout/stderr pro log ANTES do exec (fds sao herdados
    # atraves do execvp, o proprio flatpak/moonlight escreve neles direto;
    # os erros de configuracao do Apollo abaixo tambem caem aqui).
    log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    os.dup2(log_fd, 1)
    os.dup2(log_fd, 2)
    os.close(log_fd)

    try:
        result = configure_apollo(host_app_id)
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
        host = ""
        try:
            host = _load_json(os.path.join(_settings_dir(), "config.json")).get("host", "")
        except OSError:
            pass
        # Aborta em vez de tentar streamar mesmo assim - se o Apollo nao
        # foi configurado direito, o display do host provavelmente nao
        # esta' na config certa (resolucao/output errado), streamar do
        # mesmo jeito só daria uma tela quebrada em vez de erro claro.
        print(f"Falha ao configurar o Apollo: {classify_apollo_error(host, e)}", file=sys.stderr)
        sys.exit(1)
    except RuntimeError as e:
        print(f"Falha ao configurar o Apollo: {e}", file=sys.stderr)
        sys.exit(1)

    config = result["config"]

    try:
        register_with_runner(config, host_app_id, result["profile"])
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as e:
        # O Runner NAO e' mais opcional - o Apollo nao tem prep-cmd
        # nenhum, entao sem o Runner a tela do host nunca troca pro
        # target_output/resolucao certos. Abortar aqui (em vez de
        # streamar mesmo assim) da' o mesmo tratamento de erro que
        # configure_apollo ja tem - claro, no log, em vez de uma tela
        # quebrada silenciosa.
        print(f"Falha ao registrar no MoonProfile Runner (obrigatorio): {e}", file=sys.stderr)
        sys.exit(1)

    moonlight_cfg = result["profile"]["moonlight"]
    codec_flag = CODEC_FLAGS.get(moonlight_cfg["codec"], "auto")
    hdr_flag = "--hdr" if moonlight_cfg.get("hdr") else "--no-hdr"

    args = [
        "flatpak", "run", "com.moonlight_stream.Moonlight", "stream",
        config["host"], APP_NAME,
        "--resolution", moonlight_cfg["resolution"],
        "--fps", str(moonlight_cfg["fps"]),
        "--bitrate", str(moonlight_cfg["bitrate"]),
        "--video-codec", codec_flag,
        hdr_flag,
    ]

    # execvp SUBSTITUI este processo pelo flatpak (mesmo PID) - importante
    # pra Steam/Gamescope rastrearem o processo real do jogo, nao um
    # wrapper Python que fica pendurado por cima.
    os.execvp("flatpak", args)


if __name__ == "__main__":
    main()
