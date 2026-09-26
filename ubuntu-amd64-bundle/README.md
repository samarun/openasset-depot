# OpenAsset Depot Ubuntu AMD64 Bundle

This folder is ready to transfer by FTP to an Intel/AMD 64-bit Ubuntu server.
It is self-contained: the API, web, and PostgreSQL Docker images are stored as
compressed Docker archives under `images/`.

## Install

```sh
chmod +x install.sh
./install.sh depot.example.com
```

The installer verifies SHA-256 checksums, loads all three images, generates a
private `.env` with random secrets, and starts the image-only Compose stack.
Only `127.0.0.1:5173` is published; the API and PostgreSQL stay inside Docker.
To test the web gateway directly from another LAN machine, set
`OAD_WEB_BIND=0.0.0.0` in `.env` and recreate the stack. Keep port 5173 blocked
at the Internet firewall; host Nginx remains the public entry point.

Self-signup is enabled initially with `OAD_ALLOW_SIGNUPS=true`. New accounts
receive no depot access until an administrator grants a role. Set this value to
`false` and recreate `openasset-api` after onboarding the intended users.

## Configure host Nginx

```sh
sudo cp nginx.conf /etc/nginx/sites-available/openasset-depot
sudo sed -i 's/depot\.example\.com/your-domain.example/g' \
  /etc/nginx/sites-available/openasset-depot
sudo ln -sfn /etc/nginx/sites-available/openasset-depot \
  /etc/nginx/sites-enabled/openasset-depot.conf
sudo nginx -t
sudo systemctl reload nginx
sudo certbot --nginx -d your-domain.example
```

Confirm that `/etc/nginx/nginx.conf` includes `/etc/nginx/sites-enabled/*.conf`,
then confirm that the site was loaded with
`sudo nginx -T | grep -n your-domain.example`.

Then verify `https://your-domain.example/ready` and bootstrap the first admin as
described in the main deployment documentation.

## Data

Docker images contain application software only. Studio data lives in the named
volumes `openasset-depot_openasset_postgres` and
`openasset-depot_openasset_objects`; back up both volumes.
