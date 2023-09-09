# Description

This app gets a running record from Fitbit and send a report to a Mastodon instance.

# env and credentials

## .env

see .env.example

## credentials

When you access Fitbit the first time, the tokens automatically saved in credentials.json like this:

```json
{
  "access_token": "",
  "refresh_token": "",
  "expires_at": ""
}
```

