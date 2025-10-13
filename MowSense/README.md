# Mowsense

This application reads sensor data and sends it to MowMaster using UDP sockets.

### Building

<mark> build.sh </mark> builds the project and generates an OTA binary using esptool at <mark> /ota-binary/firmware.bin </mark>, which can be uploaded to OTA server.

### Monitoring
USB: espflash --monitor