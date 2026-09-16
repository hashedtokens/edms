(function () {
    const html = document.documentElement;

    function applyTheme(theme) {
        html.classList.toggle('dark', theme === 'dark');
        localStorage.setItem('theme', theme);

        const iconSun = document.getElementById('iconSun');
        const iconMoon = document.getElementById('iconMoon');

        if (iconSun && iconMoon) {
            iconSun.classList.toggle('hidden', theme !== 'dark');
            iconMoon.classList.toggle('hidden', theme === 'dark');
        }
    }

    document.addEventListener('DOMContentLoaded', () => {
        const saved = localStorage.getItem('theme') || 'dark';
        applyTheme(saved);

        const toggleBtn = document.getElementById('themeToggle');

        toggleBtn?.addEventListener('click', () => {
            const isDark = html.classList.contains('dark');
            applyTheme(isDark ? 'light' : 'dark');
        });
    });
})();