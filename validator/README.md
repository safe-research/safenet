## Validator

### Development setup

- Install dependencies
```sh
npm install
```

### Validator Docker setup

The validator worker is listening for relaying requests via Waku. To run a validator worker, you can use the provided Dockerfile.

The following commands have to be run from the project root (where `package-lock.json` is located)!

1. Setup env file
```sh
cp validator/.env.sample validator/.env
```

2. Build the image using docker or podman
```sh
podman build -f validator/Dockerfile -t validator-service .
```

3. Start the image using docker or podman
```sh
podman run -d --name shieldnet-validator --env-file validator/.env validator-service
```

4. Follow the logs
```sh
podman logs -f shieldnet-validator
```
