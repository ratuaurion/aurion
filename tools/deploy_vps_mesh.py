import subprocess

vps_cmd = """
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
ExecStart=/usr/local/bin/aurion validator start --dev --index 0 --data-dir /var/lib/aurion/data/val1.redb --p2p-bind 127.0.0.1:17447 --rpc-bind 127.0.0.1:18545 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.0.1:17448 --peer tcp/127.0.0.1:17449
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
ExecStart=/usr/local/bin/aurion validator start --dev --index 1 --data-dir /var/lib/aurion/data/val2.redb --p2p-bind 127.0.0.1:17448 --rpc-bind 127.0.0.1:18546 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.0.1:17447 --peer tcp/127.0.0.1:17449
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

cat << 'EOF' > /etc/systemd/system/aurion-val3.service
[Unit]
Description=Aurion BFT Validator 3 (Gamma - VPS Cloud)
After=network.target aurion-bootnode.service
Wants=aurion-bootnode.service

[Service]
Type=simple
User=aurion
Group=aurion
Environment="TOKIO_WORKER_THREADS=2"
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 2 --data-dir /var/lib/aurion/data/val3.redb --p2p-bind 127.0.0.1:17449 --rpc-bind 127.0.0.1:18547 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.0.1:17447 --peer tcp/127.0.0.1:17448
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable aurion-val1 aurion-val2 aurion-val3
systemctl restart aurion-val1 aurion-val2 aurion-val3
"""

subprocess.run(["ssh", "-o", "StrictHostKeyChecking=no", "root@116.212.72.89", "bash -c " + repr(vps_cmd)], check=True)
print("Configured and started all 3 validators on VPS!")
