document.addEventListener("DOMContentLoaded", () => {
    console.log("JavaScript loaded from static directory!");
    hydrate_component();
});

/**
 * Hydrate a component already loaded, like a web page.
 * Scan all the components of the page/component, and load them (indirectly recursive)
 * @param root
 * @returns {Promise<void>}
 */
async function hydrate_component(root = document) {
    // Here we are in the component _already loaded_

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

    // Look for the components to load
    const components = root.querySelectorAll("[data-component]");

    for (const el of components) {
        await load_component(el);
    }
}

/**
 * Get the component attributes from its definition
 * Load it with a POST request
 * Hydrate it to process the subcomponents
 * @param el
 * @returns {Promise<void>}
 */
async function load_component(el) {
    const url = el.getAttribute("data-component");
    const dataObject = el.getAttribute("data-object");
    const params = decodeBase64Url(dataObject ?? '');

    if (!url) return;

    try {
        // REF_TAG: POST_PARAMS :  When we do the fetch "POST"
        // we get the data-object first and decode the base64 json into a JS object.

        // const response = await fetch(url);
        const response = await fetch(url, {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: params,
        });

        if (!response.ok) throw new Error(`Failed to fetch: ${url}`);

        const html = await response.text();

        el.innerHTML = html;

        // 🔥 Recursively hydrate the newly injected content
        await hydrate_component(el);
    } catch (error) {
        console.error("Component load error:", error);
        el.innerHTML = `<p style="color: red;">Failed to load component: ${url}</p>`;
    }
}

function decodeBase64Url(base64url) {
    // Replace URL-safe characters with base64 standard characters
    let base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');

    // Add padding if necessary
    while (base64.length % 4 !== 0) {
        base64 += '=';
    }

    // Decode using atob (ASCII string)
    const decoded = atob(base64);
    return decoded;
}

function encodeBase64Url(str) {
    const base64 = btoa(str); // Encode en base64 standard

    // Convertit en base64url : remplace + et /, enlève les =
    const base64url = base64
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=+$/, '');

    return base64url;
}

/**
 * This will replace the target URL of the component and refresh it.
 * Don't forget it's a harbor application so all the business logic must be in the harbor services.
 * @param target
 * @param b64jsonData
 * @returns {Promise<void>}
 */
async function trigger_event(target, b64jsonData) {
    // Ex : relaod   'viewer' / '5aa40f74-284b-43d5-6406-13f0b9bd67e9'
    const viewer = document.getElementById(target)

    if (viewer) {
        // viewer.setAttribute('data-component', '/harbor/image/' + with_data)
        // REF_TAG: POST_PARAMS :  Change the value of the data we want to pass to the component
        viewer.setAttribute('data-object', b64jsonData)
    }

    await load_component(viewer);
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