/* SPDX-License-Identifier: GPL-3.0-or-later */

(() => {
  const carousel = document.querySelector("[data-carousel]");
  if (!carousel) return;

  const slides = [...carousel.querySelectorAll("[data-slide]")];
  const dots = [...carousel.querySelectorAll("[data-dot]")];
  const previous = carousel.querySelector("[data-prev]");
  const next = carousel.querySelector("[data-next]");
  const status = carousel.querySelector("[data-status]");

  if (slides.length < 2 || !previous || !next) return;

  let current = 0;

  const show = (index) => {
    current = (index + slides.length) % slides.length;

    slides.forEach((slide, slideIndex) => {
      const active = slideIndex === current;
      slide.hidden = !active;
      dots[slideIndex]?.setAttribute("aria-current", active ? "true" : "false");
    });

    if (status) {
      const caption = slides[current].querySelector("figcaption")?.textContent?.trim();
      status.textContent = `Screenshot ${current + 1} of ${slides.length}${caption ? `: ${caption}` : ""}`;
    }
  };

  previous.addEventListener("click", () => show(current - 1));
  next.addEventListener("click", () => show(current + 1));
  dots.forEach((dot, index) => dot.addEventListener("click", () => show(index)));

  carousel.addEventListener("keydown", (event) => {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      show(current - 1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      show(current + 1);
    }
  });

  show(0);
})();
