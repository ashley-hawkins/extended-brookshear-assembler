var cacheName = 'extended-brookshear-machine-pwa';
var filesToCache = [
  './',
  './index.html',
  './brookshear_ui.js',
  './brookshear_ui_bg.wasm',
  './favicon.ico',
  './favicon-192x192.png',
];

/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(filesToCache);
    })
  );
});

async function cachedFetch(request) {
  const cache = await caches.open(cacheName);
  return await cache.match(request);
}

/* Serve cached content when offline */
self.addEventListener('fetch', function (e) {
  e.respondWith(
    fetch(e.request, { cache: "no-cache" }).then(async function (response) {
      if (!response) return await cachedFetch(e.request);

      if (response.ok) {
        // Clone the request and response so that this response can be returned,
        // but we can also add it to the cache concurrently.
        const requestClone = e.request.clone();
        const responseClone = response.clone();
        (async function () {
          const cache = await caches.open(cacheName);
          const cachedResponse = await cache.match(requestClone);
          if (cachedResponse) {
            await cache.put(requestClone, responseClone);
          }
        })()
      }

      return response;
    }).catch(() => cachedFetch(e.request))
  );
});
