# Reverse-geocoding and POI provider survey: Google/Naver/Kakao/Korean-government, and why the 30-day cache plan needs a second look

**2026-09-08** — prompted by a question: now that POI has its own static
bake ([20260907-world-rendering-tiers.md](./20260907-world-rendering-tiers.md),
the `PoiProvider` seam in `apps/api/src/modules/world/poi/provider.ts`) and
reverse geocoding is still a live, uncached call to public Nominatim
(`use_reverse_geocode.ts`, `GEOCODE_BASE_URL`), two things came up: whether
reverse geocoding should get the same static-bake treatment as POI/terrain,
and what it'd take to add Google/Naver/Kakao/Korean-government as
alternative providers behind that seam, with results cached server-side for
30 days. This is the provider research; no code changed.

**Caveat up front:** pricing/ToS pages change and several numbers below
came from secondary sources (aggregator blogs, community Q&A threads)
rather than being confirmed against the vendor's own current legal text —
flagged per-row below. Re-verify directly before committing to any of
these, not just the pricing.

## Today

Reverse geocoding: live call to Nominatim's public instance
(`use_reverse_geocode.ts`), no cache anywhere (client or server), 1.5s
debounce to respect Nominatim's 1 req/sec usage policy — the same "not
deployed against production" gap POI had before its own bake (see
[20260816-deployment-hosting.md](./20260816-deployment-hosting.md)).

POI: baked from the same OSM source (`poi_extractor.ts`), already
statically served (`poi/v1/` in R2). Reverse geocoding baking the same way —
this session's opening suggestion — is a reasonable next step and doesn't
need a new provider to do it: it's the same OSM data (admin boundaries +
named places) this app already ingests, just a different query shape
against it (nearest/containing named area, not "features in this bbox").

## Provider survey

| Provider | Reverse geocoding | POI / place search | Free tier | Paid rate | Caching/storage terms |
| --- | --- | --- | --- | --- | --- |
| **OSM (current)** | Nominatim | `osm_elements` (already ingested) | Public instance: unlimited but throttled (1 req/s, needs identifying User-Agent) | N/A (self-hosted Nominatim has no per-call cost, only infra) | **ODbL** — caching, redistribution, and pre-baking are explicitly fine as long as attribution + share-alike on the derived data holds. This is *why* the static-bake pattern already works for POI/terrain. |
| **Google** | Geocoding API | Places API (New) | Geocoding: 10,000 calls/mo free (Essentials SKU, since the pooled $200 credit was replaced 2025-03-01). Places Pro SKU: 5,000/mo free. | Geocoding ~$5/1,000 (0–100K tier, lower at higher volume). Places Nearby/Text Search (Pro): $32/1,000 (0–100K), down to $2.40/1,000 above 5M — but any requested field like rating/hours re-prices the *whole call* to Enterprise ($35/1,000) or Enterprise+Atmosphere ($40/1,000, reviews/photos). | **Explicit, unusually specific**: place_id cacheable indefinitely; lat/lng cacheable up to 30 *consecutive calendar days*, then must delete; everything else (name, category, rating, photos, ...) may not be pre-fetched/cached/stored at all. Also: content must be displayed on an actual Google Map with attribution — a real blocker for showing Places-sourced POI on this app's own voxel map, not just a licensing cost question. |
| **Naver** (NCP Maps) | Geocoding/Reverse Geocoding, usage-billed | — | Official per-request KRW pricing sits behind NCP's pricing calculator, not found in plain docs — **not confirmed**, get a real number before estimating cost. | Not confirmed. | Not confirmed — no caching clause found in what's public; would need direct legal/docs review, not inferred from silence. |
| **Naver** (older Search API, "지역 검색"/local search) | — | Yes, general "지역 검색" | 25,000 calls/day/app | — | Being sunsetted: new app registration on the old developer center (openapi.naver.com) closed 2026-07-31; existing keys retire 2027-06-30, migrating to NCP's API Hub. Not worth building against as-is. |
| **Kakao** (Local API) | 좌표→주소 (coord2address) | 키워드/카테고리 장소 검색 | 3,000,000 calls/mo, 100,000/day | ₩0.5/call over the daily quota (coord2address; other endpoints may differ — not individually confirmed) | **Stricter than Google, no coordinate exception**: per multiple Kakao-staff answers on their own dev forum, results may not be stored in *any* form — real-time call only. Only place_id, the user's *finally-selected* place name, and place_url may be persisted; coordinates/address/phone/category may not, not even briefly. A blanket 30-day cache of Kakao results (coordinates included) would not be compliant as described. |
| **Korean government — VWorld** (국토교통부/공간정보산업진흥원) | Geocoder API 2.0 (coord→address) | — | 40,000/day documented for the address→coord direction; the reverse direction's own limit wasn't separately found | Free (government service) | **Explicit real-time-only clause**: "API 요청은 실시간으로 사용하셔야 하며 별도의 저장장치나 데이터베이스에 저장할 수 없습니다" — no storage of any kind, in the official docs, not a forum inference. Directly conflicts with a 30-day cache. |
| **Korean government — juso.go.kr** (행정안전부, 도로명주소) | 좌표제공 API (coordinate/address) | — | No documented call-count ceiling found for address search (an IP can be blocked for DDoS-shaped abuse); the coordinate-specific API's own limits weren't found in what's public | Free (government service) | Not confirmed either way — needs a direct read of `business.juso.go.kr`'s own terms before assuming it's fine, same caution as Naver above. |

## The plan this actually affects

The stated plan was: non-OSM provider results get cached server-side for 30
days once a server exists; today (no server) just call live, uncached. That
30-day figure lines up almost exactly with Google's own stated exception —
probably not a coincidence if that number came from awareness of Google's
policy — but it does not transfer to the other three commercial/government
sources surveyed:

- **Kakao and VWorld explicitly prohibit any storage.** A 30-day cache of
  either violates their stated terms as found here, full stop — not "check
  the fine print," a real conflict with the plan as described.
- **Google only exempts the coordinate pair, not the rest of a Places
  result** (name, category, address text). A 30-day cache of a full POI
  record from Google Places wouldn't be compliant even though *a* 30-day
  cache concept is explicitly Google's own.
- **Naver's terms weren't found either way** — silence isn't permission,
  needs a direct check before relying on it.
- **OSM/ODbL is the one source surveyed where a long-lived cache is
  unambiguously fine** — which is also the source this app already bakes
  POI/terrain from. Reverse geocoding baked the same way (this session's
  opening suggestion) sidesteps the whole caching-terms question rather
  than needing per-vendor legal review.

None of this rules Google/Naver/Kakao/government sources out — `PoiProvider`
was designed exactly so a commercial source can slot in later
(`provider.ts`'s own doc comment) — it means **the specific "cache
everything for 30 days" plan needs a per-field, per-vendor design** (cache
what each vendor's terms actually allow, for however long each allows it,
possibly nothing for some fields on some vendors) rather than one uniform
policy applied across all of them. That's real design work, not a detail to
patch in later.

## Recommendation

1. **Bake reverse geocoding from OSM into R2 alongside terrain/POI**, per
   this session's own opening suggestion — same data already ingested, same
   ODbL clearance the current pipeline already relies on, no new legal
   surface. The natural default provider, same as POI's OSM default today.
2. **Client stays uncached for now, exactly as already planned** — nothing
   here changes that; this only affects the future server-side cache
   design.
3. **Before wiring any commercial/government provider behind
   `PoiProvider`/a future `GeocodeProvider`, re-verify its current terms
   directly against the vendor's own legal page**, not this survey — pricing
   pages and terms of service change, and several rows above are already
   flagged as unconfirmed from public sources alone.
4. **Design the future cache layer per-field, not as one blanket duration.**
   A `{value, cached_at, ttl, source}` shape per cached field (not per whole
   record) would let, say, a Google-sourced coordinate get a 30-day TTL
   while a Google-sourced name/category gets none, and a Kakao-sourced
   anything gets none — without different vendors needing structurally
   different code paths.

## Status

Discussion only — no code changed. Numbers here are a starting point for a
real decision, not something to build against without re-confirming the
flagged rows.

**Superseded (same day) for POI specifically** by
[20260908-overture-poi-adoption.md](./20260908-overture-poi-adoption.md):
Overture Maps turned up mid-thread as a fifth option this survey didn't
originally cover, and measured out ahead of every provider above for POI —
its Places theme is licensed clean (no Google/Naver/Kakao-style display or
storage restrictions) and came back ~5x denser than OSM in two different
real Korean bboxes. The reverse-geocoding recommendation above (bake from
OSM, same as terrain) and the Google/Naver/Kakao/government findings
themselves are unaffected and still stand — only "what backs POI" changed.

Sources: [Google Geocoding API usage & billing](https://developers.google.com/maps/documentation/geocoding/usage-and-billing) · [Google Places API policies](https://developers.google.com/maps/documentation/places/web-service/policies) · [Google Maps Platform pricing](https://mapsplatform.google.com/pricing/) · [NAVER Cloud Platform Maps](https://www.ncloud.com/product/applicationService/maps) · [Kakao Local API dev guide](https://developers.kakao.com/docs/latest/ko/local/dev-guide) · [Kakao DevTalk — local API storage policy threads](https://devtalk.kakao.com/t/api/150800) · [VWorld Geocoder API 2.0 reference](https://www.vworld.kr/dev/v4dv_geocoderguide2_s001.do) · [공공데이터포털 — 국토교통부 지오코더 API](https://www.data.go.kr/data/15101106/openapi.do) · [주소기반산업지원서비스 (juso.go.kr)](https://www.juso.go.kr/addrlink/main.do)
