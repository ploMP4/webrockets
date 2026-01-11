import { $ } from "https://deno.land/x/dax@0.31.1/mod.ts";
import { chart } from "https://deno.land/x/fresh_charts/core.ts";
import { TextLineStream } from "https://deno.land/std/streams/text_line_stream.ts";

function load_test(conn, port, bytes, path) {
    const args = [conn, "0.0.0.0", port, 0, 0];
    if (bytes > 20) {
        args.push(bytes);
    }
    args.push(path);
    return $`./load_test ${args}`.stdout("piped").spawn();
}

// Parse command line arguments
const routeType = Deno.args[0] || "echo";
const validRoutes = ["echo", "db", "compute"];
if (!validRoutes.includes(routeType)) {
    console.log(`Usage: deno run --allow-all run.js [${validRoutes.join("|")}]`);
    console.log(`  Default: echo`);
    Deno.exit(1);
}

const path = `/${routeType}`;
console.log(`Running benchmarks for route: ${path}`);

const targets = [
    {
        port: 6969,
        name: "pywsrs",
    },
    // {
    //     port: 6970,
    //     name: "pywsrs-django",
    // },
    {
        port: 8000,
        name: "django-channels",
    },
    {
        port: 1234,
        name: "fastapi",
    },
    // {
    //     port: 1235,
    //     name: "django-bolt",
    // },
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
    {
        conn: 20000,
        bytes: 300,
    },
];

for (const { conn, bytes } of cases) {
    let msgResults = {};
    let connectResults = {};
    console.log(`\nTest case: ${conn} connections, ${bytes} byte payload`);

    for (const { port, name } of targets) {
        let logs = [];
        let connectMs = null;
        try {
            const client = load_test(conn, port, bytes, path);
            const readable = client.stdout().pipeThrough(new TextDecoderStream())
                .pipeThrough(new TextLineStream());
            let msgCount = 0;
            for await (const data of readable) {
                logs.push(data);
                // Capture connection time
                if (data.startsWith("Connect_ms")) {
                    connectMs = parseFloat(data.split(" ")[2]);
                }
                // Count Msg/sec samples
                if (data.startsWith("Msg/sec")) {
                    msgCount++;
                    if (msgCount >= 5) {
                        break;
                    }
                }
            }
            client.abort();
            await client;
        } catch (e) {
            // Server didn't respond or crashed
        }

        // Parse msg/sec
        const msgLines = logs.filter((line) =>
            line.length > 0 && line.startsWith("Msg/sec")
        );
        const mps = msgLines.map((line) => parseInt(line.split(" ")[1].trim()), 10);
        const avgMps = mps.length > 0 ? mps.reduce((a, b) => a + b, 0) / mps.length : 0;

        msgResults[name] = avgMps;
        connectResults[name] = connectMs || 0;

        // Display results
        const connectStr = connectMs ? `${connectMs.toFixed(0)}ms` : "N/A";
        console.log(`  ${name}: ${avgMps.toFixed(0)} msg/sec, connect: ${connectStr}`);
    }

    // Sort by msg/sec (descending - higher is better)
    msgResults = Object.fromEntries(
        Object.entries(msgResults).sort(([, a], [, b]) => b - a),
    );
    // Sort by connect time (ascending - faster is better)
    connectResults = Object.fromEntries(
        Object.entries(connectResults).sort(([, a], [, b]) => a - b),
    );

    // Generate msg/sec chart
    const msgSvg = chart({
        type: "bar",
        data: {
            labels: Object.keys(msgResults),
            datasets: [
                {
                    label: `Msg/sec - ${conn} conn, ${bytes} bytes (${routeType})`,
                    data: Object.values(msgResults),
                    backgroundColor: "rgba(54, 162, 235, 255)",
                },
            ],
        },
    });
    const msgFilename = `./${routeType}-${conn}-${bytes}-chart.svg`;
    Deno.writeTextFileSync(msgFilename, msgSvg);

    // Generate connection time chart
    const connectSvg = chart({
        type: "bar",
        data: {
            labels: Object.keys(connectResults),
            datasets: [
                {
                    label: `Connect time (ms) - ${conn} conn (${routeType})`,
                    data: Object.values(connectResults),
                    backgroundColor: "rgba(255, 99, 132, 255)",
                },
            ],
        },
    });
    const connectFilename = `./${routeType}-${conn}-${bytes}-connect-chart.svg`;
    Deno.writeTextFileSync(connectFilename, connectSvg);

    console.log(`  Charts: ${msgFilename}, ${connectFilename}`);
}
