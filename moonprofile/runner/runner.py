#!/usr/bin/env python3
"""
Runner estatico que o atalho Steam "MoonProfile Launcher" executa.

Por que existe: o Gamescope (compositor do Modo Jogo) so foca/mostra janelas
lancadas atraves do mecanismo real da Steam - um subprocess solto (o que a
Fase 1 fazia) abre em fullscreen mas fica "escondido" atras da UI, sem foco
nenhum (confirmado rodando no device). A solucao (igual o MoonDeck faz) e'
registrar este script como um atalho non-Steam; a Steam entao o executa de
verdade (Gamescope trata como jogo, foca normalmente).

Como recebe os parametros: o atalho e' sempre o mesmo (nunca muda), mas as
"Launch Options" da Steam sao reescritas a cada lancamento (pelo frontend,
via SteamClient.Apps.SetAppLaunchOptions) com variaveis de ambiente
MOONPROFILE_* - a Steam injeta essas variaveis no processo antes de rodar
este script. Ver src/steamShortcut.ts.
"""
import os
import sys


def main() -> None:
    host = os.environ.get("MOONPROFILE_HOST")
    if not host:
        print("MOONPROFILE_HOST nao definido - abortando", file=sys.stderr)
        sys.exit(1)

    app_name = os.environ.get("MOONPROFILE_APP_NAME", "SteamGame")
    resolution = os.environ.get("MOONPROFILE_RESOLUTION", "1920x1080")
    fps = os.environ.get("MOONPROFILE_FPS", "60")
    bitrate = os.environ.get("MOONPROFILE_BITRATE", "20000")
    codec = os.environ.get("MOONPROFILE_CODEC", "HEVC")
    hdr_flag = "--hdr" if os.environ.get("MOONPROFILE_HDR") == "1" else "--no-hdr"

    # Redireciona stdout/stderr pro log ANTES do exec (fds sao herdados
    # atraves do execvp, o proprio flatpak/moonlight escreve neles direto).
    log_path = os.environ.get("MOONPROFILE_LOG_PATH")
    if log_path:
        log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
        os.dup2(log_fd, 1)
        os.dup2(log_fd, 2)
        os.close(log_fd)

    args = [
        "flatpak", "run", "com.moonlight_stream.Moonlight", "stream",
        host, app_name,
        "--resolution", resolution,
        "--fps", fps,
        "--bitrate", bitrate,
        "--video-codec", codec,
        hdr_flag,
    ]

    # execvp SUBSTITUI este processo pelo flatpak (mesmo PID) - importante
    # pra Steam/Gamescope rastrearem o processo real do jogo, nao um
    # wrapper Python que fica pendurado por cima.
    os.execvp("flatpak", args)


if __name__ == "__main__":
    main()
