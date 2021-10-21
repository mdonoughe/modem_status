# modem_status

This is a small tool to workaround a firmware upgrade Comcast pushed out to my Arris SB8200 modem, after which it is not possible to check the connection status without authenticating. Sometimes the modem gets disconnected and is unable to establish a connection until after a power cycle.

The modem firmware is pretty terrible, so the process is:

1. Send HTTP Basic authorization *and* the same "user:password" token as a query string (not a query string parameter) to the status page. On success, the status page will be replaced by a token. On failure, the status page will be replaced by the login page.
2. Request the status page again, sending the token as a query string (again, not a query parameter).
3. Request logout.html to clean up resources. If you do not do this, a resource will be exhausted and you will no longer be able to log in until logout.html is requested.

The modem interface is loaded over TLS 1.2, but using a self-signed certificate that is not valid for the modem IP or even server identification. I assume all users have the same private key embedded in their firmware.

## Usage

Configure the environment variables:

- `MODEM_IP`: The IP address of the modem. The default of 192.168.100.1 is probably correct.
- `MODEM_USER`: The username for authenticating to the modem. The default of admin is probably correct.
- `MODEM_PASSWORD`: The password for authenticating to the modem. If you haven't changed it already, it is the last eight characters of the serial number printed on the bottom of the modem, in uppercase. You must log in to the modem and change the password at least once before using this program.

Run modem_status.

Make an HTTP request to http://localhost:3030/health. If configured correctly, you should get back a 200 response containing JSON data like this:

```json
{
    "acquire_downstream_channel": {
        "status": "675000000 Hz",
        "comment": "Locked"
    },
    "connectivity_state": {
        "status": "OK",
        "comment": "Operational"
    },
    "boot_state": {
        "status": "OK",
        "comment": "Operational"
    },
    "configuration_file": {
        "status": "OK",
        "comment": ""
    },
    "security": {
        "status": "Enabled",
        "comment": "BPI+"
    },
    "docsis_network_enabled": {
        "status": "Allowed",
        "comment": ""
    }
}
```

This can be added to Home Assistant using:

```yaml
binary_sensor:
  - platform: rest
    resource: http://modem-status.home.svc.cluster.local:3030/health
    name: Modem Status
    device_class: connectivity
    value_template: "{{ value_json.connectivity_state.status == 'OK' }}"
    timeout: 30
```

If you want more than just the connectivity state, you should use a single sensor to collect all the values and then use template sensors to break them out. Multiple requests to the status page are slow, and may encounter conflicts related to the logout workaround.
