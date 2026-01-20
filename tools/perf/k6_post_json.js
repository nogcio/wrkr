import http from 'k6/http';
import { check } from 'k6';

const payloadObj = {
  a: 1,
  b: 2,
  arr: [1, 2, 3],
  nested: { x: 'y', z: true },
};

const expectedJson = JSON.stringify(payloadObj);

function checksum(str) {
  let c = 0;
  for (let i = 0; i < str.length; i += 1) {
    c = (c + str.charCodeAt(i)) % 2147483647;
  }
  return c;
}

const expectedSum = checksum(expectedJson);

export default function () {
  const base = __ENV.BASE_URL;
  if (!base) {
    throw new Error('BASE_URL is required');
  }

  // Encode every iteration (to match wrkr's per-iteration json.encode cost).
  const body = JSON.stringify(payloadObj);

  const res = http.post(`${base}/echo`, body, {
    headers: {
      'content-type': 'application/json',
      accept: 'application/json',
      'x-test': '1',
    },
  });

  let decoded = null;
  try {
    decoded = JSON.parse(res.body);
  } catch (_e) {
    decoded = null;
  }

  check(res, {
    'status is 200': (r) => r.status === 200,
    'echo body matches': (r) => r.body === body,
    'checksum matches': (r) => checksum(r.body) === expectedSum,
    'encoded matches stable': () => body === expectedJson,
    'decoded.arr[3] == 3': () => decoded && decoded.arr && decoded.arr[2] === 3,
    'decoded.nested.z == true': () => decoded && decoded.nested && decoded.nested.z === true,
  });
}
