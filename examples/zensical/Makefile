# Compose cannot express a dependency on a network owned by another stack, so
# creating it is a step of its own — idempotent, so it is safe to repeat.

NETWORK ?= cloud-bridge
SUBNET  ?= 172.28.0.0/16

.PHONY: net up down logs doctor build check

net:
	@docker network inspect $(NETWORK) >/dev/null 2>&1 \
		|| docker network create --driver bridge --subnet $(SUBNET) $(NETWORK)
	@echo "network $(NETWORK) ready"

up: net
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f watcher

# Every check in the failure catalogue, as something you can run.
doctor:
	docker compose exec watcher ncpages --config /etc/ncpages/ncpages.toml doctor

# One build now, regardless of whether anything changed.
build:
	docker compose exec watcher ncpages --config /etc/ncpages/ncpages.toml build

check:
	docker compose exec watcher ncpages --config /etc/ncpages/ncpages.toml check
