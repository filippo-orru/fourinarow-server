# Four in a Row - Server

![Screenshot of main screen](screenshots/1.png)

An **online** version of the popular game **four in a row**, written in Rust on the server side and Flutter + Dart on the client.

***Download here: https://play.google.com/store/apps/details?id=ml.fourinarow***

Or play online (beta): https://play.fourinarow.ffactory.me/

## Related Projects:
- Clientside: [fourinarow-app](https://github.com/ffactory-ofcl/fourinarow-app)

- Serverside: [fourinarow-server](https://github.com/ffactory-ofcl/fourinarow-server)

- \[WIP\] bot / watcher: [fourinarow-bot](https://github.com/ffactory-ofcl/fourinarow-bot)

## Features:
- world wide online play
- over 4000 downloads
- account creation, friends system
- beautiful, minimalist design
- subtle animations
- request to battle your friends
- local mode: two players - one device

### Under the hood:
- reliable websocket connection
- message delivery guarantee
- message reordering on client and server side
- clean architecture: state and view completely separate
- automatic reconnection

![Screenshot of play selection](screenshots/2.png)

![Screenshot of play](screenshots/3.png)

# Deployment

## Prerequisites

Before getting started, make sure you have Docker with Docker Compose installed on your machine.

1. Set up reverse proxy with traefik: https://github.com/ffactory-ofcl/vps-reverse-proxy. Follow instructions there.

1. Create a deploy key using [this script](https://gist.github.com/ffactory-ofcl/a4dcfc7a68c0b8d35487aa8297e98128) and add it to the Github repository.

1. Clone this repository using the command echoed by the script.

1. Copy the `.env_template` file to `.env` and fill in the values.

    ```bash
    cp .env_template .env
    ```
    
1. Create a systemd service file:

    ```bash
    sudo cp fourinarow-server.service /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable fourinarow-server
    sudo systemctl start fourinarow-server
    ```

1. Check the status:
    
    ```bash
    sudo systemctl status fourinarow-server
    ```