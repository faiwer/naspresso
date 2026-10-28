const response = await fetch("/api/health");
const { status } = (await response.json()) as { status: string };

document.body.innerHTML = `<h1>Hello World, server is: ${status}</h1>`;
