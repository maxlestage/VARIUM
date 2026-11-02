# Le site de Varium (site/), une app Paquets. Heroku construit cette image sur la pile
# « container » (heroku.yml) ; elle tourne aussi sur n'importe quel hôte Docker :
#
#   docker build -t varium-site . && docker run --rm -p 8080:8080 varium-site

FROM rust:1-slim-bookworm AS build
WORKDIR /src
# Tout le dépôt : le site dépend de crates/varium-core, qui hérite du workspace racine.
COPY . .
WORKDIR /src/site
RUN cargo build --release --bin varium-site && cp target/release/varium-site /server

# Une petite image : le programme et ses fichiers statiques.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 varium
WORKDIR /app
COPY --from=build /server /usr/local/bin/server
COPY --from=build /src/site/public ./public
USER varium
# Heroku fixe PORT au démarrage ; 8080 ailleurs.
ENV PORT=8080
EXPOSE 8080
CMD ["server"]
