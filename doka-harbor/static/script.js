document.addEventListener("DOMContentLoaded", () => {
    console.log("JavaScript loaded from static directory!");
    hydrate_component();
});

async function hydrate_component(root = document) {
    // Here we are in the compement already loaded

    const rootElements = root.querySelectorAll(("[data-component-name]"));
    if (rootElements.length > 0) {
        console.log("Components to hydrate", rootElements[0].getAttribute("data-component-name"));
    }


    // Look for the elements to add event listeners
    const listOfElements = root.querySelectorAll("[data-action-target]");
    for (const el of listOfElements) {
        const target = el.getAttribute("data-action-target");
        const formName = el.getAttribute("data-form");
        const id = el.getAttribute("id");

        console.log("Adding event listener to element:", el.id);
        prepare_update_item_form()
    }


    // Look for the compoments to load
    const components = root.querySelectorAll("[data-component]");

    for (const el of components) {
        const url = el.getAttribute("data-component");
        if (!url) continue;

        try {
            const response = await fetch(url);
            if (!response.ok) throw new Error(`Failed to fetch: ${url}`);

            const html = await response.text();

            // Create a temporary wrapper to parse HTML
            // const temp = document.createElement("div");
            // temp.innerHTML = html;

            el.innerHTML = html;

            // Replace the element with the new content
            // el.replaceWith(...temp.childNodes);

            // 🔥 Recursively hydrate the newly injected content
            await hydrate_component(el);
        } catch (error) {
            console.error("Component load error:", error);
            el.innerHTML = `<p style="color: red;">Failed to load component: ${url}</p>`;
        }
    }
}


function prepare_update_item_form() {
    const button = document.getElementById("item_update_button");

    button.addEventListener("click", async () => {
        const formName = button.getAttribute("data-form");
        const targetUrl = button.getAttribute("data-action-target");

        if (!formName || !targetUrl) {
            console.error("Missing data-form or data-action-target attribute");
            return;
        }

        const fields = document.querySelectorAll(`[data-form="${formName}"]`);
        const data = {};

        fields.forEach((el) => {
            if (el.id) {
                data[el.id] = el.value;
            }
        });

        try {
            const encoded = CBOR.encode(data); // using cbor-x

            const response = await fetch(targetUrl, {
                method: "POST",
                headers: {
                    "Content-Type": "application/cbor",
                },
                body: encoded,
            });

            if (!response.ok) {
                throw new Error("Failed to send data");
            }

            console.log("Update successful");
        } catch (err) {
            console.error("Error updating item:", err);
        }
    });
}