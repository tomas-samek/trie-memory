## topic: spaced-repetition
Opakování s postupně rostoucími intervaly je technika, která využívá křivku zapomínání. Pokud si informaci vybavíš těsně předtím, než bys ji zapomněl, posílíš paměťovou stopu. Leitnerovy krabičky rozdělují kartičky podle úspěšnosti: správně zodpovězené postupují do vzdálenějších krabiček, chybné se vracejí na začátek. Moderní aplikace jako Anki nebo SuperMemo implementují algoritmy SM-2 a pozdější varianty. Aktivní vybavování je účinnější než pasivní čtení, protože proces hledání v paměti sám o sobě stopu posiluje. Interval se po každém úspěšném vybavení násobí faktorem, typicky mezi 1.3 a 2.5, podle obtížnosti a spolehlivosti vybavení. Když se kartička dostane do dlouhých intervalů (měsíce, roky), stává se součástí dlouhodobé paměti.

## topic: agent-messaging
Komunikace mezi agenty v časově kódovaných zprávách vychází z představy, že tick je primitivní jednotka uspořádání událostí. Každý deposit nese timestamp, observer, stream, a původ. Agenti si nevyměňují stavy přímo, pouze publikují depozity do sdíleného prostoru a ostatní je vnímají podle vlastní rezonance. Shoda v čase (koincidence ticků) implikuje vazbu; rezonance v prostoru depozitů implikuje významovou blízkost. Tick-frame ontologie tvrdí, že prostor, čas i identita jsou odvozené z kauzální struktury depozitů, ne naopak. Diskuze vedly k rozhodnutí, že verifikace této ontologie je samostatný projekt; honest-agent PoC řeší nižší patro stejného modelu.

## topic: rust-async
async fn fetch(url: &str) -> Result<String, Error> {
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    Ok(body)
}

Asynchronní programování v Rustu se opírá o budoucnosti (Future) a runtime jako tokio nebo async-std. Klíčové slovo async vytváří stavový automat; .await předává řízení runtimu, dokud operace neskončí. Borrow checker platí i přes .await body, takže půjčky napříč čekáním musí respektovat lifetime. Pin a Unpin zajišťují, že sebe-referenční stavy se nepřesunou v paměti. Send a Sync určují, jestli budoucnost může přejít mezi vlákny multi-threaded runtime. Channels z tokio::sync (mpsc, oneshot, broadcast) jsou hlavní nástroj pro předávání dat mezi úlohami.

## topic: cooking-chicken
Pečené kuře s bramborami a rozmarýnem patří mezi klasické nedělní pokrmy. Kuře o váze 1.5 kg omyj a osuš, potři směsí soli, pepře, sladké papriky a prolisovaného česneku. Dovnitř vlož čerstvý rozmarýn, citron rozkrojený na čtvrtky a pár stroužků česneku. Brambory oloupej, nakrájej na větší kusy a rozlož do pekáče spolu s kořenovou zeleninou (mrkev, petržel, pastinák). Pekáč zakryj alobalem a peč na 180 stupňů 45 minut, poté odkryj a peč dalších 20-30 minut do zezlátnutí kůže. Během pečení kuře dvakrát přelij výpekem; tím kůže zkaramelizuje a brambory nasáknou šťávu.

## topic: music-theory
The circle of fifths organizes the twelve pitches of Western music into a geometric shape that reveals key relationships. Moving clockwise by perfect fifths: C, G, D, A, E, B, F-sharp, C-sharp, G-sharp, D-sharp, A-sharp, F, and back to C. Each step adds one sharp to the key signature; moving counter-clockwise adds flats. Adjacent keys share six of seven notes, which is why modulating to a neighbor sounds smooth. Dominant seventh chords resolve to the tonic a fifth below — the V7-I progression is the engine of functional harmony. Secondary dominants apply the same trick to non-tonic chords, temporarily borrowing tension from outside the key.

## topic: tides
Tides on Earth arise from the gravitational pull of the Moon and Sun on the ocean, combined with the rotation of the Earth beneath that pull. The Moon stretches the water into a slight bulge on the side facing it and another on the opposite side; as the Earth rotates, each coast passes through these bulges twice per day, giving the familiar two high tides and two low tides. The Sun adds a smaller effect that constructively sums at new and full moons (spring tides) and partially cancels at quarter moons (neap tides). Local geography — bay shape, shelf depth, coastline orientation — amplifies or dampens the astronomical signal, so tide ranges vary from centimeters in the open ocean to fifteen meters in the Bay of Fundy.

## topic: sql-queries
SELECT user_id, COUNT(*) AS order_count, SUM(total_cents) / 100.0 AS total_dollars
FROM orders
WHERE created_at >= '2026-01-01'
  AND status IN ('paid', 'shipped')
GROUP BY user_id
HAVING COUNT(*) >= 3
ORDER BY total_dollars DESC
LIMIT 100;

Aggregations group rows by a key and compute summary statistics. HAVING filters on the aggregate; WHERE filters on individual rows before grouping. Window functions provide per-row context without collapsing the row set: ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY created_at) gives each order a running index within its user. Common table expressions (WITH clauses) break queries into named steps. Indexing strategy matters: a composite index on (user_id, created_at) accelerates both the filter and the grouping. EXPLAIN ANALYZE reveals the planner's choices and estimated row counts.

## topic: japanese-greetings
日本語で自己紹介をするとき、まず名前を言います。「はじめまして、私はタカハシです」と言えば丁寧です。次に出身地を伝えます。「東京から来ました」は一般的な表現です。仕事や学校の話も続けます。「大学で物理学を勉強しています」または「ソフトウェア会社で働いています」。最後に相手との関係を締めくくります。「どうぞよろしくお願いします」はどんな場面でも使える定型句です。フォーマルな場面では敬語を使い、友人同士ではもっと短い表現で十分です。「おはよう」は朝、「こんにちは」は昼、「こんばんは」は夕方の挨拶です。

## topic: linear-algebra
A vector in n-dimensional space is an ordered tuple (x_1, x_2, ..., x_n). Addition is componentwise; scalar multiplication scales every component uniformly. The dot product a·b = sum(a_i * b_i) gives a scalar; it equals |a||b|cos(theta) where theta is the angle between the vectors. Orthogonal vectors have zero dot product. A matrix is a rectangular array of numbers that represents a linear map from one vector space to another. Multiplying a matrix A (m-by-n) times a vector x (n-by-1) yields a vector (m-by-1). Matrix multiplication is associative, not commutative. The determinant of a square matrix is nonzero if and only if the matrix is invertible; it also measures the signed volume scaling applied by the map.

## topic: config-json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8080,
    "tls": {
      "enabled": true,
      "cert_path": "/etc/ssl/server.pem",
      "key_path": "/etc/ssl/server.key"
    }
  },
  "database": {
    "url": "postgres://user:pass@localhost:5432/app",
    "pool_size": 32,
    "statement_timeout_ms": 5000
  },
  "logging": {
    "level": "info",
    "format": "json",
    "destinations": ["stdout", "/var/log/app.log"]
  },
  "features": {
    "new_dashboard": true,
    "legacy_export": false,
    "rate_limit_per_minute": 120
  }
}

## topic: czech-idioms
V českém jazyce se pro popis pracovitosti často používají přirovnání k živočichům a přírodním jevům. "Pracuje jako kůň" znamená, že někdo dře bez ohledu na únavu. "Makat od nevidím do nevidím" říká, že pracuje od tmy do tmy. "Zabrat", "zapřáhnout se", "dát tomu všechno" — všechna tato slovesa popisují intenzivní nasazení. Naopak lenost má také bohaté přirovnání: "válet se", "flákat se", "ulejvat se z práce". "Tlouct špačky" je archaické spojení pro podřimování, doslovně "bít vrabce". "Dělat něco levou zadní" paradoxně neznamená lajdáctví, nýbrž mistrovské zvládnutí bez námahy.

## topic: gravity-short
Gravity is the mutual attraction that every massive body exerts on every other massive body. Newton described it as a force proportional to the product of the masses and inversely proportional to the square of their separation. Einstein reinterpreted it as the curvature of spacetime caused by energy and momentum: massive bodies follow geodesics in the curved manifold, and what we perceive as gravitational attraction is simply inertia along those paths. General relativity reduces to Newtonian gravity in the weak-field, slow-motion limit, but it additionally predicts phenomena that Newton cannot: gravitational lensing of light, precession of planetary orbits, frame dragging, gravitational waves, and the existence of black holes with event horizons from which nothing escapes.
