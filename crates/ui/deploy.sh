#!/bin/sh
set -e
bunx wrangler pages deploy dist --project-name brookshear
rsync -v -au --info=progress2 ./dist/ $DEPLOY_HOST:~/public_html/bm/
