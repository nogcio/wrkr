import http from 'k6/http';

export default function () {
  const base = __ENV.BASE_URL;
  if (!base) {
    throw new Error('BASE_URL is required');
  }

  http.get(`${base}/hello`);
}
