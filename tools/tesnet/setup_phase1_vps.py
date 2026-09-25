import subprocess
import sys

vps_script = """
cat << 'EOF' > /etc/systemd/system/aurion-val1.service
[Unit]
Description=Aurion BFT Validator 1 (Alpha - VPS Cloud)
After=network.target aurion-bootnode.service
Wants=aurion-bootnode.service

[Service]
Type=simple
User=aurion
Group=aurion
Environment="TOKIO_WORKER_THREADS=2"
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 0 --data-dir /var/lib/aurion/data/val1.redb --p2p-bind 116.212.72.89:17447 --rpc-bind 127.0.0.1:18545 --bootnode tcp/127.0.0.1:7447 --peer tcp/116.212.72.89:17448
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

cat << 'EOF' > /etc/systemd/system/aurion-val2.service
[Unit]
Description=Aurion BFT Validator 2 (Beta - VPS Cloud)
After=network.target aurion-bootnode.service
Wants=aurion-bootnode.service

[Service]
Type=simple
User=aurion
Group=aurion
Environment="TOKIO_WORKER_THREADS=2"
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 1 --data-dir /var/lib/aurion/data/val2.redb --p2p-bind 116.212.72.89:17448 --rpc-bind 127.0.0.1:18546 --bootnode tcp/127.0.0.1:7447 --peer tcp/116.212.72.89:17447
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

ufw allow 17447/tcp
ufw allow 17448/tcp
systemctl daemon-reload
systemctl restart aurion-val1 aurion-val2
echo "SUCCESS_VPS_PHASE1_CONFIGURED"
"""

import base64

b64_script = base64.b64encode(vps_script.replace('\r\n', '\n').encode()).decode()
cmd = ["wsl.exe", "ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", f"echo {b64_script} | base64 -d | bash"]
res = subprocess.run(cmd, text=True, capture_output=True)
print(res.stdout)
if res.stderr:
    print("STDERR:", res.stderr)
