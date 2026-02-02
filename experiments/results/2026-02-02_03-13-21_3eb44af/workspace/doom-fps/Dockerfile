
FROM node:18-alpine

WORKDIR /app

# Ensure npm is available (diagnostic - should be pre-installed)
RUN apk add --no-cache npm

COPY package.json ./

RUN npm install

COPY . .

EXPOSE 3000

CMD [ "node", "server.js" ]
