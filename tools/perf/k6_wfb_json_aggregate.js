import http from 'k6/http';
import { check } from 'k6';

const countries = ['US', 'DE', 'FR', 'JP'];
const categories = ['Electronics', 'Books', 'Clothing', 'Home'];

const OrderStatus = {
  COMPLETED: 1,
  PENDING: 2,
  FAILED: 3,
};

const statuses = [
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.COMPLETED,
  OrderStatus.PENDING,
  OrderStatus.FAILED,
  OrderStatus.PENDING,
];

function prngInt(seed, min, max) {
  const x = (seed * 1103515245 + 12345) % 2147483647;
  const span = (max - min) + 1;
  return min + (x % span);
}

function initZeroMap(keys) {
  const out = {};
  for (const k of keys) {
    out[k] = 0;
  }
  return out;
}

function generateCase(caseId) {
  const numOrders = 100;

  const orders = [];
  let expectedProcessed = 0;
  const expectedResults = initZeroMap(countries);
  const expectedCategoryStats = initZeroMap(categories);

  const clientId = `client-${caseId}`;

  for (let i = 0; i < numOrders; i += 1) {
    const status = statuses[i % statuses.length];
    const country = countries[i % countries.length];

    const items = [];
    let orderAmount = 0;

    for (let j = 0; j < 3; j += 1) {
      const seed = (caseId * 100000) + (i * 10) + j;
      const price = prngInt(seed, 1000, 10000);
      const quantity = prngInt(seed + 7, 1, 5);
      const category = categories[(i + j) % categories.length];

      orderAmount += price * quantity;
      items.push({
        quantity,
        category,
        price_cents: price,
      });

      if (status === OrderStatus.COMPLETED) {
        expectedCategoryStats[category] += quantity;
      }
    }

    orders.push({
      id: `${i + 1}`,
      status,
      country,
      items,
    });

    if (status === OrderStatus.COMPLETED) {
      expectedProcessed += 1;
      expectedResults[country] += orderAmount;
    }
  }

  return {
    client_id: clientId,
    orders,
    expected_processed: expectedProcessed,
    expected_results: expectedResults,
    expected_category_stats: expectedCategoryStats,
  };
}

function totalsMatch(actual, expected) {
  if (!actual || !expected) {
    return false;
  }
  for (const k of Object.keys(expected)) {
    if (Number(actual[k] ?? 0) !== expected[k]) {
      return false;
    }
  }
  return true;
}

let caseId = 0;

export default function () {
  const base = __ENV.BASE_URL;
  if (!base) {
    throw new Error('BASE_URL is required');
  }

  caseId += 1;
  const data = generateCase(caseId);

  const body = JSON.stringify({
    client_id: data.client_id,
    orders: data.orders,
  });

  const res = http.post(`${base}/analytics/aggregate`, body, {
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
    'decoded ok': () => decoded !== null,
    'echoed_client_id matches': () => decoded && decoded.echoed_client_id === data.client_id,
    'processed_orders matches': () => decoded && Number(decoded.processed_orders) === data.expected_processed,
    'amount_by_country matches': () => decoded && totalsMatch(decoded.amount_by_country, data.expected_results),
    'quantity_by_category matches': () => decoded && totalsMatch(decoded.quantity_by_category, data.expected_category_stats),
  });
}
