import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/deploying-aws')({
  component: TemplatesDeployingAwsGuide,
})

function TemplatesDeployingAwsGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose max-w-none">
        <h1>Deploying to AWS</h1>
        <p className="lead">
          <code>deploy/terraform/</code> is a reference deployment of the API to
          AWS (workspace ADR 0025), the same in both templates. It is a
          starting point you own: read it, size it, change it. The engine's
          deploying guide covers what any deployment needs (configuration, the
          pool, health checks, migrations); this page is the AWS one.
        </p>

        <h2>What it makes</h2>
        <table>
          <thead>
            <tr><th>Module</th><th></th></tr>
          </thead>
          <tbody>
            <tr><td><code>network</code></td><td>a VPC over two zones; public subnets (the load balancer, one NAT gateway), private ones (the tasks, the database); a security group each for the load balancer, the tasks and the database</td></tr>
            <tr><td><code>database</code></td><td>RDS PostgreSQL 17, encrypted, backed up 7 days; its password generated and handed on only inside <code>DATABASE_URL</code></td></tr>
            <tr><td><code>storage</code></td><td>the uploads bucket, private and encrypted, with CORS for direct uploads from <code>cors_allowed_origins</code></td></tr>
            <tr><td><code>mail</code></td><td>SES: the sending domain's identity, DKIM, and SMTP credentials for <code>SMTP_URL</code></td></tr>
            <tr><td><code>secrets</code></td><td>Secrets Manager: <code>JWT_SECRET</code> and <code>TOTP_ENCRYPTION_KEY</code> generated, <code>DATABASE_URL</code> and <code>SMTP_URL</code> from the modules above</td></tr>
            <tr><td><code>service</code></td><td>ECR; an ECS Fargate cluster with a <code>web</code> service (<code>APP_ROLE=web</code>, behind an application load balancer checking <code>/health/ready</code>) and a <code>worker</code> service (<code>APP_ROLE=worker</code>); a <code>migrate</code> task definition; CloudWatch logs; the roles (the tasks may use the bucket and nothing else)</td></tr>
          </tbody>
        </table>
        <p>
          <code>modules/app</code> wires them together. <code>envs/staging</code>
          and <code>envs/prod</code> call it with their sizes; prod's database
          is multi-AZ and protected from <code>terraform destroy</code>. Both
          keep their state in S3 (<code>use_lockfile</code>, no DynamoDB table
          needed). The tasks stop the way the API drains: ECS gives them 40
          seconds (<code>SHUTDOWN_DELAY_SECS</code> 5 plus
          <code>SHUTDOWN_GRACE_SECS</code> 25, and a margin), and the load
          balancer deregisters a stopping task after 30.
        </p>

        <h2>First deploy</h2>
        <ol>
          <li>
            <strong>The state bucket</strong>, once per account: in
            <code>deploy/terraform/bootstrap</code>,
            <code>terraform init && terraform apply -var bucket=&lt;name&gt;</code>.
            Put the bucket's name in <code>envs/staging/main.tf</code>'s (and
            prod's) <code>backend</code> block.
          </li>
          <li>
            <strong>The settings</strong>: <code>envs/staging/terraform.tfvars</code>
            (ignored by git):
            <pre><code>{`image_tag            = "2026-10-30.1"
app_url              = "https://app.example.com"
mail_domain          = "example.com"
mail_from            = "Example <no-reply@example.com>"
cors_allowed_origins = ["https://app.example.com"]
certificate_arn      = "arn:aws:acm:us-east-1:123456789012:certificate/…"`}</code></pre>
          </li>
          <li>
            <strong>The images</strong>, built and pushed before the first
            apply, so the services have something to run: see <strong>Each
            release</strong> below, with the repository URL from
            <code>terraform apply -target
            module.app.module.service.aws_ecr_repository.this</code>.
          </li>
          <li>
            <code>terraform init && terraform apply</code> in
            <code>envs/staging</code>. Commit the
            <code>.terraform.lock.hcl</code> the first <code>init</code> writes.
          </li>
          <li>
            <strong>Mail</strong>: publish the <code>mail_dns_records</code> it
            prints (a verification TXT and three DKIM CNAMEs), and ask AWS to
            take the account out of the SES sandbox before mailing anyone but
            verified addresses.
          </li>
          <li>
            <strong>The migrations</strong>, before the services take traffic:
            run the <code>migrate_command</code> it prints (below).
          </li>
        </ol>

        <h2>Each release</h2>
        <pre><code>{`$ REPO=$(terraform output -raw repository_url)   # in envs/staging
$ TAG=2026-10-30.2
$ docker build -t $REPO:$TAG .
$ docker build --target migrate -t $REPO:$TAG-migrate .
$ aws ecr get-login-password | docker login --username AWS --password-stdin \${REPO%%/*}
$ docker push $REPO:$TAG && docker push $REPO:$TAG-migrate
$ terraform apply -var image_tag=$TAG -target module.app.module.service.aws_ecs_task_definition.this
$ eval "$(terraform output -raw migrate_command)"   # wait for exit code 0
$ terraform apply -var image_tag=$TAG`}</code></pre>
        <p>The migrations run before the new version starts, so write each one
        to work with the version still running (add a column, deploy, then use
        it; never rename in one step). The repository's tags are immutable: a
        tag is one build. ECS rolls the services, and rolls back a deployment
        whose tasks fail their health checks.</p>

        <h2>Testing it without AWS</h2>
        <p>
          <code>envs/local</code> applies the parts
          <a href="https://floci.io" target="_blank" rel="noreferrer">Floci</a>
          emulates — the bucket, the secrets, mail (the SES identity and the
          SMTP user), the database (a real PostgreSQL container) — and
          <code>terraform test</code> checks them: the bucket's name, the four
          secrets (and that the generated ones read back the right size), the
          database's address, the DNS records. Then it destroys them.
        </p>
        <pre><code>{`$ docker compose up -d floci                       # the project's root
$ cd deploy/terraform/envs/local
$ terraform init && terraform test`}</code></pre>
        <p>
          CI runs it, with <code>terraform fmt -check</code>, <code>validate</code>
          on every environment and <code>tflint</code>. What it does not
          prove: the network, the load balancer and ECS are only validated,
          and an emulator is not AWS. The first apply to a real staging
          account is the last check, every time the Terraform changes.
        </p>

        <h2>Costs and sizing</h2>
        <p>
          The fixed costs of an idle staging are the NAT gateway, the load
          balancer and the smallest RDS instance; Fargate is per task-hour.
          One NAT gateway serves both zones: a zone's outage cuts the tasks in
          the other zone off the internet (not off the database); one gateway
          per zone removes that, at twice the cost. Size
          <code>service.cpu</code> and <code>memory</code> from the
          <code>http_request_duration_seconds</code> p95 and the pool gauges
          under load, the <code>worker_count</code> from
          <code>{"jobs_queued{state=\"due\"}"}</code>, and the database from its CPU
          and connections (<code>DB_MAX_CONNECTIONS</code> × tasks stays under
          its limit).
        </p>
      </article>
    </div>
  )
}