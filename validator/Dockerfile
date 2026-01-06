# ---- Base Image ----
# Use a specific, slim version of Node for reproducibility and a smaller image.
FROM node:24-slim AS base
WORKDIR /usr/src/app

# ---- Builder Stage ----
# This stage builds the TypeScript source code into JavaScript.
FROM base AS builder

# First, copy over package files and install all dependencies.
# This includes devDependencies needed for the build process (e.g., typescript).
COPY package.json package-lock.json ./
# IMPORTANT: Copy the package.json for every workspace
COPY validator/package.json validator/package.json
RUN npm ci --workspace=validator

# Then, copy over the rest of the validator source code and build the project.
# The .dockerignore file will prevent node_modules on the host from being copied.
COPY validator/ validator/
RUN npm run build --workspace=validator

# ---- Production Stage ----
# This stage creates the final, lean image for running the application.
FROM base AS runner

# Copy package.json and install *only* production dependencies.
# This avoids including devDependencies in the final image, keeping it small.
COPY --from=builder /usr/src/app/package.json /usr/src/app/package-lock.json ./
# CRITICAL: You must copy the package.json for *every* workspace
# that 'validator' depends on, plus 'validator' itself.
# This allows 'npm ci' to resolve the workspace structure.
COPY --from=builder /usr/src/app/validator/package.json validator/
RUN npm ci --workspace=validator --omit=dev --ignore-scripts

# Copy the compiled code from the builder stage.
COPY --from=builder /usr/src/app/validator/dist validator/dist

# Additionally, `better-sqlite3` builds native code which is needed when
# running, so make sure to copy that as well. This is a hacky work-around for
# now, but should be revisited in the future.
COPY --from=builder /usr/src/app/node_modules/better-sqlite3/build node_modules/better-sqlite3/build

# Copy the container entry point script.
COPY validator/bin/entrypoint.sh validator/bin/entrypoint.sh

# Set the Node environment to "production"
ENV NODE_ENV=production

# Specify the command to run the compiled worker.
CMD [ "validator/bin/entrypoint.sh" ]
