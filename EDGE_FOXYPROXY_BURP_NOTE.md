# Edge + FoxyProxy + Burp Quick Note

FoxyProxy is already installed in Microsoft Edge.

## Recommended Setup

Use Edge as:

- normal browsing with proxy off
- testing only with FoxyProxy turned on

This keeps unrelated traffic out of Burp and reduces noise.

## Burp Proxy Settings

In Burp, confirm the proxy listener is running on:

- host: `127.0.0.1`
- port: `8080`

Default place to check:

- `Proxy > Options`

## FoxyProxy Settings In Edge

Create or verify a proxy entry with:

- proxy type: `HTTP`
- host: `127.0.0.1`
- port: `8080`

Then choose one of these operating modes:

1. Global proxy through Burp
2. Only selected target domains through Burp

For most testing, target-domain rules are cleaner than global proxying.

## HTTPS Interception

If you want Burp to intercept HTTPS traffic cleanly:

1. Open Burp
2. Export or open Burp's CA certificate
3. Import that certificate into Windows / Edge as a trusted root certificate
4. Restart Edge if needed

Without the Burp CA certificate, many HTTPS sites will fail certificate validation while proxied.

## Quick Verification

1. Turn FoxyProxy on for the target.
2. Browse to the target site in Edge.
3. Check Burp:
   - `Proxy > HTTP history`
4. Confirm requests appear there.

If they do, Edge is flowing through Burp correctly.

## Practical Usage Advice

- Keep FoxyProxy off when not actively testing.
- Prefer a dedicated Edge profile for Burp-heavy work if you want cleaner separation later.
- If a site breaks, first check whether the Burp CA certificate is trusted.
- If no traffic appears in Burp, check the FoxyProxy host/port and Burp listener settings first.
