import subprocess

nginx_conf = """server {
    server_name bootnode.ratuaurion.store;

    add_header Cache-Control "no-store, no-cache, must-revalidate, proxy-revalidate, max-age=0" always;
    add_header Pragma "no-cache" always;

    location /rpc {
        proxy_pass http://127.0.0.1:18545/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 60s;
    }

    location /api/v1/transactions/recent {
        proxy_pass http://127.0.0.1:18545/api/v1/transactions/recent;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /api/v1/blocks/latest {
        proxy_pass http://127.0.0.1:18545/api/v1/blocks/latest;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    location /api/v1/peers {
        proxy_pass http://127.0.0.1:8080/api/v1/peers;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }


    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;

        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }

    listen 443 ssl;
    ssl_certificate /etc/letsencrypt/live/bootnode.ratuaurion.store/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/bootnode.ratuaurion.store/privkey.pem;
    include /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;
}

server {
    if ($host = bootnode.ratuaurion.store) {
        return 301 https://$host$request_uri;
    }

    listen 80;
    server_name bootnode.ratuaurion.store;
    return 404;
}
"""

cmd = ["wsl", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", "cat > /etc/nginx/sites-available/aurion-bootnode && nginx -t && systemctl reload nginx"]
res = subprocess.run(cmd, input=nginx_conf, text=True, capture_output=True)
print("STDOUT:", res.stdout)
print("STDERR:", res.stderr)
