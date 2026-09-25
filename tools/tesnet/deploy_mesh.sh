#!/bin/bash
set -e

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
ExecStart=/usr/local/bin/aurion validator start --dev --index 0 --data-dir /var/lib/aurion/data/val1.redb --p2p-bind 127.0.1.1:17447 --rpc-bind 127.0.0.1:18545 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.2.1:17448 --peer tcp/127.0.3.1:17449 --peer tcp/127.0.4.1:17450
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
ExecStart=/usr/local/bin/aurion validator start --dev --index 1 --data-dir /var/lib/aurion/data/val2.redb --p2p-bind 127.0.2.1:17448 --rpc-bind 127.0.0.1:18546 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.1.1:17447 --peer tcp/127.0.3.1:17449 --peer tcp/127.0.4.1:17450
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
ExecStart=/usr/local/bin/aurion validator start --dev --index 2 --data-dir /var/lib/aurion/data/val3.redb --p2p-bind 127.0.3.1:17449 --rpc-bind 127.0.0.1:18547 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.1.1:17447 --peer tcp/127.0.2.1:17448 --peer tcp/127.0.4.1:17450
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

cat << 'EOF' > /etc/systemd/system/aurion-val4.service
[Unit]
Description=Aurion BFT Validator 4 (Delta - VPS Cloud)
After=network.target aurion-bootnode.service
Wants=aurion-bootnode.service

[Service]
Type=simple
User=aurion
Group=aurion
Environment="TOKIO_WORKER_THREADS=2"
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 3 --data-dir /var/lib/aurion/data/val4.redb --p2p-bind 127.0.4.1:17450 --rpc-bind 127.0.0.1:18548 --bootnode tcp/127.0.0.1:7447 --peer tcp/127.0.1.1:17447 --peer tcp/127.0.2.1:17448 --peer tcp/127.0.3.1:17449
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl restart aurion-val1 aurion-val2 aurion-val3 aurion-val4
echo "DISTINCT_SUBNET_MESH_DEPLOYED"
