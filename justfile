fusor := env_var_or_default("FUSOR_BIN", "fusor")

default:
    @just --list

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked

site-check:
    cargo fmt --manifest-path apps/Cargo.toml --all -- --check
    cargo clippy --manifest-path apps/Cargo.toml --workspace --all-targets --locked -- -D warnings
    cargo test --manifest-path apps/Cargo.toml --workspace --locked

[working-directory: "apps"]
site:
    {{fusor}} build --site --locked

preview port="4187":
    {{fusor}} preview apps/dist --port {{port}}

landing-dev:
    {{fusor}} dev --manifest-path apps/landing/Cargo.toml

docs-dev:
    {{fusor}} dev --manifest-path apps/docs/Cargo.toml

capture-preview:
    python3 scripts/capture-preview.py

site-browser:
    node scripts/site.mjs

setup-browser:
    cargo install fusor-cli --version 0.1.4 --locked
    {{fusor}} install --manifest-path apps/Cargo.toml -p sql-bomb-docs --locked
    npm ci
    npx playwright install chromium

[unix]
deploy environment="production":
    @case "{{environment}}" in \
        production|preview) ;; \
        *) echo "deploy: expected production or preview" >&2; exit 2 ;; \
    esac
    vercel build {{ if environment == "production" { "--prod" } else { "" } }} --yes
    find .vercel/output/static -name '.fusor-*.json' -delete
    vercel deploy --prebuilt {{ if environment == "production" { "--prod" } else { "" } }} --yes
