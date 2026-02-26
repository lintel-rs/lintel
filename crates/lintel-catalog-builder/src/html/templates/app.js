(function () {
  "use strict";

  var BASE = document.querySelector('meta[name="base-url"]');
  var BASE_URL = BASE ? BASE.getAttribute("content") : "/";

  // --- Theme toggle ---
  var THEMES = ["auto", "dark", "light"];
  var toggle = document.getElementById("theme-toggle");

  function getTheme() {
    return localStorage.getItem("theme") || "auto";
  }

  function setTheme(t) {
    document.documentElement.setAttribute("data-theme", t);
    localStorage.setItem("theme", t);
  }

  setTheme(getTheme());

  if (toggle) {
    toggle.addEventListener("click", function () {
      var cur = getTheme();
      var idx = THEMES.indexOf(cur);
      setTheme(THEMES[(idx + 1) % THEMES.length]);
    });
  }

  // --- Search ---
  var searchInput = document.getElementById("search-input");
  var searchResults = document.getElementById("search-results");
  var searchIndex = null;
  var activeIdx = -1;

  function loadIndex() {
    if (searchIndex) return Promise.resolve(searchIndex);
    return fetch(BASE_URL + "search-index.json")
      .then(function (r) {
        return r.json();
      })
      .then(function (data) {
        searchIndex = data;
        return data;
      });
  }

  function renderResults(items) {
    if (!items.length) {
      searchResults.hidden = true;
      return;
    }
    activeIdx = -1;
    var html = "";
    for (var i = 0; i < items.length && i < 20; i++) {
      var it = items[i];
      html +=
        '<a class="search-result-item" href="' +
        BASE_URL +
        escapeAttr(it.u) +
        '">';
      html += '<div class="search-result-name">' + escapeHtml(it.n) + "</div>";
      if (it.d)
        html +=
          '<div class="search-result-desc">' + escapeHtml(it.d) + "</div>";
      if (it.g)
        html +=
          '<div class="search-result-group">' + escapeHtml(it.g) + "</div>";
      html += "</a>";
    }
    searchResults.innerHTML = html;
    searchResults.hidden = false;
  }

  function escapeHtml(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function escapeAttr(s) {
    return s
      .replace(/&/g, "&amp;")
      .replace(/"/g, "&quot;")
      .replace(/</g, "&lt;");
  }

  function doSearch(q) {
    if (!q || !searchIndex) {
      searchResults.hidden = true;
      return;
    }
    var lower = q.toLowerCase();
    var matched = searchIndex.filter(function (it) {
      return (
        it.n.toLowerCase().indexOf(lower) !== -1 ||
        (it.d && it.d.toLowerCase().indexOf(lower) !== -1) ||
        (it.f && it.f.toLowerCase().indexOf(lower) !== -1)
      );
    });
    renderResults(matched);
  }

  function updateActive(items) {
    for (var i = 0; i < items.length; i++) {
      items[i].classList.toggle("active", i === activeIdx);
    }
  }

  if (searchInput) {
    searchInput.addEventListener("focus", function () {
      loadIndex();
    });
    searchInput.addEventListener("input", function () {
      loadIndex().then(function () {
        doSearch(searchInput.value.trim());
      });
    });
    searchInput.addEventListener("keydown", function (e) {
      var items = searchResults.querySelectorAll(".search-result-item");
      if (e.key === "ArrowDown") {
        e.preventDefault();
        activeIdx = Math.min(activeIdx + 1, items.length - 1);
        updateActive(items);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        activeIdx = Math.max(activeIdx - 1, -1);
        updateActive(items);
      } else if (e.key === "Enter" && activeIdx >= 0 && items[activeIdx]) {
        e.preventDefault();
        items[activeIdx].click();
      } else if (e.key === "Escape") {
        searchResults.hidden = true;
        searchInput.blur();
      }
    });
    document.addEventListener("click", function (e) {
      if (
        !searchInput.contains(e.target) &&
        !searchResults.contains(e.target)
      ) {
        searchResults.hidden = true;
      }
    });
  }

  // --- Keyboard shortcut: "/" to focus search ---
  document.addEventListener("keydown", function (e) {
    if (
      e.key === "/" &&
      document.activeElement !== searchInput &&
      document.activeElement.tagName !== "INPUT" &&
      document.activeElement.tagName !== "TEXTAREA"
    ) {
      e.preventDefault();
      if (searchInput) searchInput.focus();
    }
  });

  // --- Page outline (sticky TOC for schema detail pages) ---
  var outline = document.getElementById("page-outline");
  if (outline) {
    var headings = document.querySelectorAll(".schema-detail h2[id]");
    if (headings.length >= 2) {
      var html = '<div class="outline-title">On this page</div>';
      for (var h = 0; h < headings.length; h++) {
        html +=
          '<a class="outline-link" href="#' +
          headings[h].id +
          '">' +
          escapeHtml(headings[h].textContent) +
          "</a>";

        // List individual properties under the Properties heading
        if (headings[h].id === "properties") {
          var propItems = document.querySelectorAll(
            ".property-item[id^='prop-']",
          );
          for (var p = 0; p < propItems.length; p++) {
            var propName = propItems[p].querySelector(".property-name");
            if (propName) {
              html +=
                '<a class="outline-link outline-sub" href="#' +
                propItems[p].id +
                '">' +
                escapeHtml(propName.textContent) +
                "</a>";
            }
          }
        }

        // List individual definitions under the Definitions heading
        if (headings[h].id === "definitions") {
          var defBlocks = document.querySelectorAll(".definition-block[id]");
          for (var d = 0; d < defBlocks.length; d++) {
            var defName = defBlocks[d].querySelector(".def-name");
            if (defName) {
              html +=
                '<a class="outline-link outline-sub" href="#' +
                defBlocks[d].id +
                '">' +
                escapeHtml(defName.textContent) +
                "</a>";
            }
          }
        }
      }
      outline.innerHTML = html;

      // Track which section is currently visible (h2 headings only)
      var outlineLinks = outline.querySelectorAll(
        ".outline-link:not(.outline-sub)",
      );
      var currentActive = null;

      var observer = new IntersectionObserver(
        function (entries) {
          entries.forEach(function (entry) {
            if (entry.isIntersecting) {
              if (currentActive) currentActive.classList.remove("active");
              for (var i = 0; i < outlineLinks.length; i++) {
                if (
                  outlineLinks[i].getAttribute("href") ===
                  "#" + entry.target.id
                ) {
                  outlineLinks[i].classList.add("active");
                  currentActive = outlineLinks[i];
                  break;
                }
              }
            }
          });
        },
        { rootMargin: "-10% 0px -80% 0px" },
      );

      for (var j = 0; j < headings.length; j++) {
        observer.observe(headings[j]);
      }
    }
  }
})();
