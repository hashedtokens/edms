document.addEventListener("DOMContentLoaded", () => {

    const buttons =
        document.querySelectorAll(".navbar button");

    const currentPage =
        document.body.dataset.currentPage || "";

    const activeClasses = [
        "bg-cyan-500",
        "text-white",
        "shadow-lg",
        "shadow-cyan-500/30"
    ];

    const inactiveClasses = [
        "text-slate-400"
    ];


    // ========================================================
    // SET ACTIVE BUTTON
    // ========================================================

    function setActiveButton(button) {

        buttons.forEach(btn => {

            btn.classList.remove(
                ...activeClasses
            );

            btn.classList.add(
                ...inactiveClasses
            );

        });


        if (!button) return;


        button.classList.remove(
            ...inactiveClasses
        );

        button.classList.add(
            ...activeClasses
        );

    }


    // ========================================================
    // INITIAL ACTIVE STATE
    // ========================================================

    buttons.forEach(button => {

        const page =
            button.dataset.page;

        if (
            page &&
            page === currentPage
        ) {

            setActiveButton(button);

        }

    });


    // ========================================================
    // NAVIGATION
    // ========================================================

    buttons.forEach(button => {

        button.addEventListener(
            "click",
            event => {

                event.preventDefault();


                const target =
                    button.dataset.target;

                const page =
                    button.dataset.page;


                // ------------------------------------------------
                // Already on this page
                // ------------------------------------------------

                if (
                    page &&
                    page === currentPage
                ) {

                    return;

                }


                // ------------------------------------------------
                // No target
                // ------------------------------------------------

                if (
                    !target
                ) {

                    console.warn(
                        "Navigation target is missing:",
                        button
                    );

                    return;

                }


                // ------------------------------------------------
                // Visual feedback
                // ------------------------------------------------

                setActiveButton(
                    button
                );


                // ------------------------------------------------
                // Navigate
                // ------------------------------------------------

                window.setTimeout(
                    () => {

                        window.location.assign(
                            target
                        );

                    },
                    120
                );

            }
        );

    });

});