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

export default function () {
  const target = __ENV.GRPC_TARGET;
  if (!target) {
    throw new Error('GRPC_TARGET is required');
  }

  if (!connected) {
    client.connect(target, { plaintext: true });
    connected = true;
  }

  client.invoke('wrkr.test.EchoService/Echo', { message: 'ping' });
}
