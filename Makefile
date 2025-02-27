MAKEFLAGS += --jobs
MAKEFLAGS += --no-builtin-rules
MAKEFLAGS += --warn-undefined-variables
SHELL := bash
.DELETE_ON_ERROR:
.ONESHELL:
.SHELLFLAGS := --norc --noprofile -Eeuo pipefail -O dotglob -O nullglob -O failglob -O globstar -c

.DEFAULT_GOAL := dev

.PHONY: clean clobber lint mypy clippy deps build release ci test

OS ?=

ifeq ($(OS),Windows_NT)
VENV := Scripts
else
VENV := bin
endif

clean:
	shopt -u failglob
	rm -v -rf -- artifacts/*.{zip,deb} .mypy_cache/ target/ temp/

clobber: clean
	rm -v -rf -- .venv/

.venv/$(VENV)/pip:
	python -m venv -- .venv

.venv/$(VENV)/mypy: .venv/$(VENV)/pip
	'$<' install --upgrade --requirement requirements.txt -- mypy types-PyYAML types-toml types-Jinja2

mypy: .venv/$(VENV)/mypy
	'$<' -- .

dev:
	cargo build

clippy:
	cargo clippy --all-targets --all-features

lint: mypy clippy

deps: .venv/$(VENV)/mypy
	.venv/$(VENV)/python ./build.py deps

build: lint test
	.venv/$(VENV)/python ./build.py build

release: .venv/$(VENV)/mypy
	.venv/$(VENV)/python ./build.py buildr -- "$$TRIPLE"

ci: .venv/$(VENV)/mypy
	.venv/$(VENV)/python ./ci/release.py

test:
	cargo test --locked
