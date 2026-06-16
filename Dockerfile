# Ferrum 鐵鏈 白皮書 — static site
# Serves the self-contained index.html via nginx on port 8088.
#
#   docker build -t ferrum-whitepaper .
#   docker run --rm -p 8088:8088 ferrum-whitepaper
#   open http://localhost:8088
FROM nginx:1.27-alpine

# Site config: listen on 8088 instead of the default 80.
RUN rm -f /etc/nginx/conf.d/default.conf
COPY nginx.conf /etc/nginx/conf.d/ferrum.conf

# The whitepaper page.
COPY index.html /usr/share/nginx/html/index.html

EXPOSE 8088

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD wget -q -O /dev/null http://127.0.0.1:8088/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
