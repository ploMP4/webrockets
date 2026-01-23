FROM node:25-alpine AS build
RUN npm install -g pnpm
WORKDIR /app
COPY docs/package.json docs/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY docs/ .
RUN pnpm build

FROM nginx:alpine
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
