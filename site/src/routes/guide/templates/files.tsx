import { Link, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/guide/templates/files')({
  component: TemplatesFilesGuide,
})

function TemplatesFilesGuide() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <article className="prose dark:prose-invert max-w-none">
        <h1>Files and storage (the templates)</h1>
        <p>
          The templates (<code>rok sqlc init --template actix</code> or
          <code>axum</code>) store files two ways: <strong>uploads</strong>
          (<code>/api/v1/media</code>, the hand-written <code>media</code>
          feature), any allowed file a signed-in user keeps, and
          <strong>profile photos</strong> (<code>/api/v1/users/me/avatar</code>,
          in the <code>account</code> feature), one small JPEG per user. Both
          store their bytes through one port, <code>Storage</code>, chosen by
          <code>STORAGE_URL</code>. The decisions are in engine ADR 0022; the
          accounts and API keys these routes check are on
          <Link to="/guide/templates/auth">Accounts and sign-in</Link>.
        </p>

        <h2>Where the code is</h2>
        <table>
          <thead>
            <tr><th>Path</th><th>What it holds</th></tr>
          </thead>
          <tbody>
            <tr><td><code>src/features/media/</code></td><td>the uploads: list, upload, metadata, content, delete</td></tr>
            <tr><td><code>src/features/account/</code></td><td>the profile photo: set, remove, and the public <code>{"GET /avatars/{file}"}</code></td></tr>
            <tr><td><code>src/shared/storage/</code></td><td>the <code>Storage</code> port and <code>ObjectStorage</code>, its implementation over <code>object_store</code>; <code>StorageUrl</code></td></tr>
            <tr><td><code>src/shared/thumbnail.rs</code></td><td>decoding a photo under limits, cropping it, re-encoding it as JPEG</td></tr>
            <tr><td><code>src/shared/body.rs</code></td><td>reading an upload's body, and answering the wire standard's <code>413</code></td></tr>
            <tr><td><code>queries/media.sql</code>, <code>queries/account.sql</code></td><td>the features' SQL</td></tr>
            <tr><td><code>schema/0001_init.sql</code></td><td>the <code>media</code> table; <code>users.avatar_url</code></td></tr>
          </tbody>
        </table>

        <p>
          <code>dto.rs</code> and <code>service.rs</code> of both features,
          <code>shared/storage/</code> and <code>shared/thumbnail.rs</code> are
          the same file in both templates; handlers, routes and
          <code>shared/body.rs</code> are written for the framework.
        </p>

        <h2>Uploads</h2>
        <p>
          Every route needs a signed-in caller (a session or an API key), and
          every file is its owner's alone: another user's file is a
          <code>404</code>, as a missing one is.
        </p>
        <table>
          <thead>
            <tr><th>Endpoint</th><th>Answers</th></tr>
          </thead>
          <tbody>
            <tr><td><code>POST /media</code></td><td><code>201</code> the file's metadata; <code>400</code>, <code>413</code>, <code>415</code>, <code>422</code></td></tr>
            <tr><td><code>GET /media</code></td><td><code>200</code> the caller's files, newest first (<code>?page=&per_page=</code>)</td></tr>
            <tr><td><code>{"GET /media/{id}"}</code></td><td><code>200</code> the metadata, <code>404</code></td></tr>
            <tr><td><code>{"GET /media/{id}/content"}</code></td><td><code>200</code> the bytes, <code>404</code></td></tr>
            <tr><td><code>{"DELETE /media/{id}"}</code></td><td><code>204</code>, <code>404</code></td></tr>
          </tbody>
        </table>

        <h3>Uploading</h3>
        <p>The body is the raw file, not a form: its <code>Content-Type</code> is ignored. Name it with <code>?filename=</code> or the <code>X-Filename</code> header (255 characters at most, <code>422</code> past it); the query wins when both are sent.</p>
        <pre><code>{`curl -X POST "$API/api/v1/media?filename=report.pdf" \
  -H "Authorization: Bearer $TOKEN" --data-binary @report.pdf`}</code></pre>
        <p>An empty body is a <code>400</code>, one over
        <code>MEDIA_MAX_UPLOAD_BYTES</code> a <code>413</code>. The type is
        detected from the file's first bytes (<code>infer</code>), never from
        its name or the request's headers, and must be one of
        <code>MEDIA_ALLOWED_TYPES</code>: otherwise <code>415
        unsupported_media_type</code>. The bytes are stored first, under a key
        the server makes (<code>media/&lt;uuid&gt;</code>), then the row; if the row
        fails, the bytes are deleted.</p>

        <h3>Serving</h3>
        <p>
          <code>{"GET /media/{id}/content"}</code> answers the bytes with the
          detected <code>Content-Type</code>,
          <code>X-Content-Type-Options: nosniff</code> (so a browser never
          guesses another type) and
          <code>Content-Security-Policy: default-src 'none'; sandbox</code> (so
          a document opened anyway runs nothing). An image is served
          <code>Content-Disposition: inline</code>; anything else
          <code>attachment</code>, with a cleaned file name, so it downloads
          instead of opening in your origin.
        </p>

        <h3>Deleting</h3>
        <p>Removes the row, then the bytes. A store that fails to delete them is logged, not answered: the row is gone, and the orphaned object can be cleaned up later.</p>

        <h2>Profile photos</h2>
        <p>
          <code>PUT /users/me/avatar</code> takes a raw JPEG, PNG or WebP image
          of at most 2 MiB and answers the updated user (<code>200</code>). The
          image is decoded under limits (at most 8192×8192 pixels and 128 MiB of
          memory, so a small file claiming a huge size is refused before it is
          allocated), cropped to its centre, scaled to 256×256, transparency
          flattened onto white, and re-encoded as a JPEG: EXIF, GPS positions
          and anything else the original carried are gone. A file that isn't
          one of those formats, can't be decoded, or is over the limits is a
          <code>422</code>; over 2 MiB, a <code>413</code>. GIF isn't
          accepted: one frame of an animation is not what a user picks.
        </p>
        <p>
          The JPEG is stored as <code>avatars/&lt;random&gt;.jpg</code>, and the
          previous one is deleted once the new one is committed.
          <code>DELETE /users/me/avatar</code> removes it. Every user response
          answers <code>avatar_url</code>: <code>/api/v1/avatars/&lt;file&gt;</code>,
          or <code>null</code>. That route is public (an <code>&lt;img&gt;</code>
          tag sends no token) and answers <code>image/jpeg</code> with
          <code>Cache-Control: public, max-age=31536000, immutable</code>: a
          new photo gets a new name, so a cached one is never stale. The name
          is random, so it can't be guessed from the user's id.
        </p>
        <p>
          <code>avatar_url</code> is a generated column of <code>users</code>
          (<code>'/api/v1/' || avatar_key</code>), so the generated
          <code>users</code> feature answers it too. Moving the API off
          <code>/api/v1</code> means changing that column as well.
        </p>

        <h2>Storage</h2>
        <p>
          Features store bytes through the <code>Storage</code> port in
          <code>src/shared/storage/</code>, an <code>async_trait</code> with
          three methods: <code>put(key, bytes, content_type)</code>,
          <code>get(key)</code>, answering
          <code>{"Option<StoredObject { bytes, content_type }>"}</code>, and
          <code>delete(key)</code>. A missing key is <code>Ok(None)</code>,
          deleting one is no error, and a store that fails is an
          <code>AppError::Internal</code> (a <code>500</code>, the cause
          logged). Keys are made by the server, never taken from a request, and
          <code>is_valid_key</code> refuses any that could leave the store.
          <code>get</code> answers the whole object rather than a stream: every
          upload is capped, and bytes keep the port free of either framework's
          body type.
        </p>

        <p>
          <code>ObjectStorage</code> implements it over the <code>object_store</code>
          crate, chosen by <code>STORAGE_URL</code>:
        </p>
        <table>
          <thead>
            <tr><th><code>STORAGE_URL</code></th><th>Where the bytes go</th></tr>
          </thead>
          <tbody>
            <tr><td><code>file://./storage</code> (the default)</td><td>a <code>storage/</code> directory under the working directory, created when missing</td></tr>
            <tr><td><code>file:///srv/uploads</code>, <code>file:///C:/uploads</code></td><td>an absolute directory</td></tr>
            <tr><td><code>s3://&lt;bucket&gt;</code></td><td>an S3 bucket, or any S3-compatible store</td></tr>
            <tr><td><code>memory://</code></td><td>memory, lost when the process stops: for tests</td></tr>
          </tbody>
        </table>
        <p>
          <code>AppState::new</code> opens the store, so every application
          built from it stores where it is configured; a store that can't be
          opened stops startup. A local store keeps no content type, so it is
          detected again from the bytes when a file is read; S3 and memory keep
          the one it was put with. To store somewhere else, implement
          <code>Storage</code> and pass it to
          <code>AppState::with_storage</code>.
        </p>

        <h2>S3 and S3-compatible stores</h2>
        <p>Set <code>STORAGE_URL=s3://&lt;bucket&gt;</code>; the rest comes from the standard <code>AWS_*</code> variables:</p>
        <pre><code>{`STORAGE_URL=s3://my-app-uploads
AWS_REGION=eu-west-1
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...`}</code></pre>
        <p>For an S3-compatible store (MinIO, Cloudflare R2, Backblaze B2, a
        local MinIO in development), add its endpoint, and
        <code>AWS_ALLOW_HTTP=true</code> for one without TLS:</p>
        <pre><code>{`STORAGE_URL=s3://uploads
AWS_ENDPOINT=http://localhost:9000
AWS_ALLOW_HTTP=true
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin`}</code></pre>
        <p>The bucket isn't reached at startup: wrong credentials or a wrong
        region show on the first upload, as a <code>500</code> with the cause
        in the log. Give the credentials only <code>s3:PutObject</code>,
        <code>s3:GetObject</code> and <code>s3:DeleteObject</code> on the
        bucket. Keep the bucket private: every file is served through the API,
        which checks its owner.</p>

        <h2>Settings</h2>
        <table>
          <thead>
            <tr><th>Variable</th><th>Default</th><th>Setting</th></tr>
          </thead>
          <tbody>
            <tr><td><code>STORAGE_URL</code></td><td><code>file://./storage</code></td><td>where uploaded bytes go</td></tr>
            <tr><td><code>MEDIA_MAX_UPLOAD_BYTES</code></td><td><code>10485760</code> (10 MiB)</td><td>the largest upload, in bytes</td></tr>
            <tr><td><code>MEDIA_ALLOWED_TYPES</code></td><td><code>image/jpeg,image/png,image/webp,image/gif,application/pdf</code></td><td>the types an upload may have, detected from its bytes, comma-separated</td></tr>
          </tbody>
        </table>
        <p><code>.env.example</code> lists them; <code>src/config.rs</code>
        reads them. An invalid <code>STORAGE_URL</code> or a number that isn't
        one stops startup with one line on stderr.</p>

        <h2>Limits</h2>
        <table>
          <thead>
            <tr><th>What</th><th>Limit</th><th>Past it</th></tr>
          </thead>
          <tbody>
            <tr><td>An upload</td><td><code>MEDIA_MAX_UPLOAD_BYTES</code> (10 MiB)</td><td><code>413 payload_too_large</code></td></tr>
            <tr><td>A profile photo</td><td>2 MiB</td><td><code>413 payload_too_large</code></td></tr>
            <tr><td>A photo's pixels</td><td>8192×8192, 128 MiB to decode</td><td><code>422 validation_failed</code></td></tr>
            <tr><td>A file name</td><td>255 characters</td><td><code>422 validation_failed</code></td></tr>
            <tr><td>A JSON body</td><td>2 MiB</td><td><code>413 payload_too_large</code></td></tr>
          </tbody>
        </table>

        <h2>Testing</h2>
        <p>The tests configure <code>STORAGE_URL=memory://</code>
        (<code>tests/support::config</code>), so they never touch the disk.
        <code>tests/support::Recorded</code> is a <code>Storage</code> that
        records which keys hold an object, swapped in for a test that checks
        what was stored or deleted; <code>tests/media.rs</code> and
        <code>tests/avatars.rs</code> use it.</p>
      </article>
    </div>
  )
}