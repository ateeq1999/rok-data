# ADR 0025: an AWS reference deployment, tested against Floci

Date: 2026-10-29 · Status: accepted

Each template ships Terraform under `deploy/terraform/` for one target:
AWS ECS Fargate, with `web` and `worker` services from one image (workspace
ADR 0024's `APP_ROLE`), RDS Postgres, S3, SES, Secrets Manager and
CloudWatch, and a one-off task that runs `rok migrate` before each deploy.
It is tested without an AWS account against Floci, an MIT-licensed AWS
emulator that needs no token: `envs/local` points the provider's
endpoints at it, `terraform test` applies the whole stack, and a smoke run
drives the app image against what was provisioned. Floci also replaces
MinIO in the templates' compose stack, so local dev, CI and the
infrastructure test see the same endpoints. The Floci image is pinned to
an exact version.

## Considered Options

- **App Runner**: simpler, but no separate worker process.
- **EKS**: too much cluster for a starter.
- **LocalStack**: reported to need an account token for its community edition; Floci is a drop-in on the same port (4566) with no account.

## Consequences

- Floci passing is not AWS parity; a real `staging` apply stays the last check.
- The Terraform is framework-independent, so `template_drift` covers it.
- Other clouds are recipes, not second reference deployments.
