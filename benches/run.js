import { $ } from "https://deno.land/x/dax@0.31.1/mod.ts";
import { chart } from "https://deno.land/x/fresh_charts/core.ts";
import { TextLineStream } from "https://deno.land/std/streams/text_line_stream.ts";

function wait(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function load_test(conn, port, bytes) {
    return $`./load_test ${conn} 0.0.0.0 ${port} 0 0 ${bytes}`
        .stdout("piped").spawn();
}

const targets = [
    {
        port: 6969,
        name: "pywsrs",
    },
    {
        port: 8000,
        name: "django-channels",
    },
    {
        port: 1234,
        name: "fastapi",
    },
    {
        port: 1235,
        name: "django-bolt",
    },
    {
        port: 4200,
        name: "python-websockets",
    },
];

const cases = [
    {
        conn: 100,
        bytes: 20,
    },
    {
        conn: 10,
        bytes: 1024,
    },
    {
        conn: 10,
        bytes: 16 * 1024,
    },
    {
        conn: 200,
        bytes: 16 * 1024,
    },
    {
        conn: 500,
        bytes: 16 * 1024,
    },
    {
        conn: 10000,
        bytes: 1024,
    },
];

for (const { conn, bytes } of cases) {
    let results = {};
    for (const { port, name, server, arg } of targets) {
        let logs = [];
        try {
            // const proc = $`${server} ${arg || ""}`.spawn();
            // console.log(`Waiting for ${name} to start...`);
            // await wait(1000);

            const client = load_test(conn, port, bytes == 20 ? "" : bytes);
            const readable = client.stdout().pipeThrough(new TextDecoderStream())
                .pipeThrough(new TextLineStream());
            let count = 0;
            for await (const data of readable) {
                logs.push(data);
                count++;
                if (count === 5) {
                    break;
                }
            }
            client.abort();
            // proc.abort();
            // await proc;
            await client;
        } catch (e) {
            console.log(e);
        }

        const lines = logs.filter((line) =>
            line.length > 0 && line.startsWith("Msg/sec")
        );
        const mps = lines.map((line) => parseInt(line.split(" ")[1].trim()), 10);
        const avg = mps.reduce((a, b) => a + b, 0) / mps.length;
        results[name] = avg;
    }

    results = Object.fromEntries(
        Object.entries(results).sort(([, a], [, b]) => b - a),
    );

    const title = `Connections: ${conn}, Payload size: ${bytes}`;
    const svg = chart({
        type: "bar",
        data: {
            labels: Object.keys(results),
            datasets: [
                {
                    label: title,
                    data: Object.values(results),
                    backgroundColor: [
                        "rgba(54, 162, 235, 255)",
                    ],
                },
            ],
        },
    });

    Deno.writeTextFileSync(`./${conn}-${bytes}-chart.svg`, svg);
}
