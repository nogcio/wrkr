import grpc from 'k6/net/grpc';

const client = new grpc.Client();
// k6 resolves proto import paths relative to the script directory in some setups.
// Try the local path first (when running from tools/perf), then fall back to repo-root relative.
try {
  client.load(['protos'], 'echo.proto');
} catch (_e) {
  client.load(['tools/perf/protos'], 'echo.proto');
}

let connected = false;

function normalizeGrpcTarget(raw) {
  if (!raw) {
    return raw;
  }

  // k6/net/grpc expects "host:port" (no scheme).
  // Allow passing http(s)://host:port for consistency with wrkr BASE_URL.
  const m = raw.match(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\/([^/]+)(?:\/.*)?$/);
  if (m) {
    return m[1];
  }

  // Also tolerate accidentally passing a URL with a path but no scheme.
  return raw.split('/')[0];
}

export default function () {
  const target = normalizeGrpcTarget(__ENV.BASE_URL);
  if (!target) {
    throw new Error('BASE_URL is required');
  }

  if (!connected) {
    client.connect(target, { plaintext: true });
    connected = true;
  }

  client.invoke('wrkr.test.EchoService/Echo', { message: 'ping' });
}
