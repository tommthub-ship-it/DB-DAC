.PHONY: help build up down dev logs clean

help:
	@echo "간지-DAC 명령어"
	@echo "──────────────────────────────"
	@echo "make build    빌드"
	@echo "make up       운영 모드 실행"
	@echo "make dev      개발 모드 실행 (로컬 PG + 로그 뷰어 포함)"
	@echo "make down     중지"
	@echo "make logs     실시간 로그"
	@echo "make clean    컨테이너 + 볼륨 전체 삭제"

build:
	docker compose build

up:
	docker compose up -d

dev:
	docker compose --profile dev up -d
	@echo ""
	@echo "✅ 간지-DAC 개발 모드 실행 중"
	@echo "  PostgreSQL 프록시 → localhost:15432"
	@echo "  Admin API         → http://localhost:8080"
	@echo "  로그 뷰어          → http://localhost:9999"

down:
	docker compose --profile dev down

logs:
	docker compose logs -f proxy api

clean:
	docker compose --profile dev down -v --remove-orphans
