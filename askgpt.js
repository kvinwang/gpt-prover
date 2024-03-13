const config = JSON.parse(scriptArgs[0]);
const MODEL = scriptArgs[1];
const PROMPT = scriptArgs[2];

const ENDPOINT = config.openai.url;
const API_KEY = config.openai.apiKey;

async function main() {
    console.log(`OpenAI API: ${ENDPOINT}`);
    console.log(`OpenAI API_KEY: ${API_KEY}`);
    const response = await fetch(ENDPOINT, {
        method: 'POST',
        headers: {
            'Accept': 'application/json, text/plain, */*',
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${API_KEY}`,
        },
        body: encode({
            "model": MODEL,
            "messages": [{ "role": "user", "content": PROMPT }],
            "stream": false
        })
    });

    if (!response.ok) {
        return JSON.stringify({
            status: response.status,
        });
    }

    const reply = await response.text();
    return JSON.stringify({
        api: ENDPOINT,
        model: MODEL,
        prompt: PROMPT,
        status: response.status,
        reply: reply,
    });
}

function encode(o) {
    return new TextEncoder().encode(JSON.stringify(o))
}

main()
    .then(reply => scriptOutput = reply)
    .catch(err => scriptOutput = err.toString())
    .finally(Sidevm.exit);
