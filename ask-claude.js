const secret = JSON.parse(secretData);
const MODEL = scriptArgs[0];
const PROMPT = scriptArgs[1];
const ENDPOINT = secret.url;
const API_KEY = secret.apiKey;

console.log = Sidevm.inspect;

async function main() {
    const response = await fetch(ENDPOINT, {
        method: 'POST',
        headers: {
            'Accept': 'application/json, text/plain, */*',
            'Content-Type': 'application/json',
            'anthropic-version': '2023-06-01',
            'x-api-key': `${API_KEY}`,
        },
        body: encode({
            "max_tokens": 4096,
            "model": MODEL,
            "messages": [{ "role": "user", "content": PROMPT }],
        })
    });

    if (!response.ok) {
        return JSON.stringify({
            status: response.status,
        });
    }

    const reply = await response.text();
    console.log(reply);
    return JSON.stringify({
        api: ENDPOINT,
        model: MODEL,
        prompt: PROMPT,
        status: response.status,
        reply,
    });
}

function encode(o) {
    return new TextEncoder().encode(JSON.stringify(o))
}

main()
    .then(reply => scriptOutput = reply)
    .catch(err => scriptOutput = err.toString())
    .finally(Sidevm.exit);
