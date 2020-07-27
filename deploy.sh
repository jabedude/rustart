#!/bin/bash

set -ex

cargo b
vagrant ssh -c "sudo systemctl stop systemd-journald.service systemd-journald-audit.socket systemd-journald.socket systemd-journald-dev-log.socket && sudo cp /vagrant/target/debug/logd /bin/logd && sudo systemctl restart systemd-journald.service"
