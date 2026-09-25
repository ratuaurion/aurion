val2 = """[Unit]
Description=Aurion BFT Validator 2 (Beta - WSL Local)
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ratu
Group=ratu
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 1 --data-dir /var/lib/aurion/data/val2.redb --p2p-bind 172.19.145.171:17448 --rpc-bind 127.0.0.1:18546 --bootnode tcp/116.212.72.89:7447
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
"""

val3 = """[Unit]
Description=Aurion BFT Validator 3 (Gamma - WSL Local)
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ratu
Group=ratu
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/aurion validator start --dev --index 2 --data-dir /var/lib/aurion/data/val3.redb --p2p-bind 172.19.145.171:17447 --rpc-bind 127.0.0.1:18545 --bootnode tcp/116.212.72.89:7447
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
"""

with open('/etc/systemd/system/aurion-val2.service', 'w') as f:
    f.write(val2)
with open('/etc/systemd/system/aurion-val3.service', 'w') as f:
    f.write(val3)
print("Updated systemd service files.")
