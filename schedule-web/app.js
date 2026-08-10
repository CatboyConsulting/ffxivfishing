if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("./service-worker.js").catch(() => {
      // The app remains usable when service workers are unavailable.
    });
  });
}

import init, {
  init_default,
  get_fish,
  list_all_fish_info,
  get_fish_next_window,
  get_fish_windows_in_schedule,
  unix_to_eorzea_esec,
  unix_from_eorzea_time,
} from "./pkg/ffxivfishing_wasm.js";

const DAY_OPTIONS = [
  { value: "", label: "Any" },
  { value: "1", label: "Mon" },
  { value: "2", label: "Tue" },
  { value: "3", label: "Wed" },
  { value: "4", label: "Thu" },
  { value: "5", label: "Fri" },
  { value: "6", label: "Sat" },
  { value: "0", label: "Sun" },
];
const FILTER_INTUITION = true;

let _savedOpen = true;

let _lastWindows = [];
let _lastFishName = "";
let _selectedFishId = 0;
let _queryGeneration = 0;
let _wasmReady = false;
let _notificationTimerId = null;
let _fishHydrationGen = 0;
let _fishHydrationActive = false;

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => {
    const entities = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#39;",
    };
    return entities[char];
  });
}

function fishEyesIndicator(fishWindow) {
  return fishWindow?.fishEyes === true
    ? '<span class="fish-eyes-indicator" role="img" aria-label="Only available with Fish Eyes" title="Only available with Fish Eyes">&#128065;</span>'
    : "";
}

function daySelectHtml(selected) {
  return DAY_OPTIONS.map(
    (o) =>
      `<option value="${o.value}"${o.value === selected ? " selected" : ""}>${o.label}</option>`,
  ).join("");
}

function timeToSecs(str) {
  str = String(str).trim();
  let match = str.match(/^(\d{1,2}):(\d{2})\s*(AM|PM)?$/i);
  if (!match) return NaN;
  let h = parseInt(match[1], 10);
  const m = parseInt(match[2], 10);
  const ampm = match[3] ? match[3].toUpperCase() : null;
  if (m > 59) return NaN;
  if (ampm && (h < 1 || h > 12)) return NaN;
  if (!ampm && (h > 24 || (h === 24 && m !== 0))) return NaN;
  if (ampm === "PM" && h !== 12) h += 12;
  if (ampm === "AM" && h === 12) h = 0;
  return h * 3600 + m * 60;
}

function secsToTime(secs) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return String(h).padStart(2, "0") + ":" + String(m).padStart(2, "0");
}

function formatRealDuration(seconds) {
  const total = Number(seconds);
  if (!Number.isFinite(total) || total < 0) return "";
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  return (
    (hours ? `${hours}h` : "") +
    (minutes ? `${minutes}m` : "") +
    (secs || (!hours && !minutes) ? `${secs}s` : "")
  );
}

function formatLocalWindow(start, end) {
  const startDate = start.toLocaleDateString();
  const endDate = end.toLocaleDateString();
  const endDatePrefix = startDate === endDate ? "" : `${endDate} `;
  return `${startDate} ${start.toLocaleTimeString()} - ${endDatePrefix}${end.toLocaleTimeString()}`;
}

function fmtWhen(diffSec, endDiffSec) {
  if (endDiffSec !== undefined && endDiffSec <= 0)
    return { whenStr: "ended", state: "past" };
  if (diffSec <= 0) return { whenStr: "now", state: "ongoing" };
  let whenStr;
  if (diffSec < 60) whenStr = "<1m";
  else if (diffSec < 3600) whenStr = `${Math.floor(diffSec / 60)}m`;
  else if (diffSec < 86400) whenStr = `${Math.floor(diffSec / 3600)}h`;
  else whenStr = `${Math.floor(diffSec / 86400)}d`;
  return { whenStr, state: "future" };
}

function fmtSavedWhen(startUnix, endUnix, nowUnix) {
  const { whenStr, state } = fmtWhen(startUnix - nowUnix, endUnix - nowUnix);
  if (state === "future" && startUnix - nowUnix < 60) {
    return { displayWhen: `in ${startUnix - nowUnix}s`, state };
  }
  if (state === "ongoing") {
    const remaining = endUnix - nowUnix;
    const remainingStr =
      remaining < 60 ? `${remaining}s` : `${Math.floor(remaining / 60)}m`;
    return { displayWhen: `ending in ${remainingStr}`, state };
  }
  return {
    displayWhen: state === "future" ? "in " + whenStr : whenStr,
    state,
  };
}

function isAlwaysUp(fish) {
  return Number.isFinite(fish.fishUptime) && fish.fishUptime >= 1;
}

function isPrerequisiteAlwaysUp(prerequisite) {
  return (
    !!prerequisite &&
    eoTime(prerequisite.startDisplay) === "00:00" &&
    eoTime(prerequisite.endDisplay) === "00:00"
  );
}

let _nextWindowCacheMinute = -1;
const _nextWindowCache = new Map();

function getNextFishWindow(id, nowUnix, nowEorzea) {
  const minute = Math.floor(nowUnix / 60);
  if (minute !== _nextWindowCacheMinute) {
    _nextWindowCacheMinute = minute;
    _nextWindowCache.clear();
  }
  const useFishEyes = Boolean(
    document.getElementById("use-fish-eyes")?.checked,
  );
  const cacheKey = `${id}:${useFishEyes}`;
  if (_nextWindowCache.has(cacheKey)) return _nextWindowCache.get(cacheKey);
  try {
    const nextJson = get_fish_next_window(
      id,
      nowEorzea,
      FILTER_INTUITION,
      useFishEyes,
    );
    const next = JSON.parse(nextJson);
    _nextWindowCache.set(cacheKey, next);
    return next;
  } catch {
    _nextWindowCache.set(cacheKey, null);
    return null;
  }
}

function eoTime(s) {
  const parts = String(s).trim().split(/\s+/);
  if (parts.length < 2) return String(s);
  return parts[1].split(":").slice(0, 2).join(":");
}

const STORAGE_KEY = "lastSearch";

function saveSearch(p) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
  } catch { }
}

function restoreSearch() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const p = JSON.parse(raw);
    const validSchedule = (entry) =>
      entry &&
      Number.isFinite(entry.startSec) &&
      Number.isFinite(entry.endSec) &&
      entry.startSec >= 0 &&
      entry.startSec <= 86400 &&
      entry.endSec >= 0 &&
      entry.endSec <= 86400 &&
      (entry.dayOfWeek === undefined ||
        (Number.isInteger(entry.dayOfWeek) &&
          entry.dayOfWeek >= 0 &&
          entry.dayOfWeek <= 6));
    if (
      p &&
      typeof p.fishId === "number" &&
      Array.isArray(p.schedule) &&
      p.schedule.every(validSchedule) &&
      typeof p.days === "number" &&
      Number.isFinite(p.days) &&
      (p.limit === undefined ||
        (typeof p.limit === "number" && Number.isFinite(p.limit))) &&
      (p.useFishEyes === undefined || typeof p.useFishEyes === "boolean")
    ) {
      return p;
    }
  } catch { }
  return null;
}

function getFormData() {
  const saved = restoreSearch();
  if (saved) {
    return saved;
  }
  return {
    fishId: 0,
    schedule: [{ startSec: 0, endSec: 86400 }],
    days: 30,
    limit: 100,
    useFishEyes: false,
  };
}

function createScheduleRow(entry) {
  const div = document.createElement("div");
  div.className = "schedule-row";
  div.innerHTML = `
  <label>Day
    <select>${daySelectHtml(entry.dayOfWeek !== undefined ? String(entry.dayOfWeek) : "")}</select>
  </label>
  <label>Start
    <input type="text" value="${escapeHtml(secsToTime(entry.startSec))}" />
  </label>
  <label>End
    <input type="text" value="${escapeHtml(secsToTime(entry.endSec))}" />
  </label>
  <button type="button" class="remove-btn" title="Remove" aria-label="Remove schedule">&#10005;</button>
`;
  div.querySelector(".remove-btn").addEventListener("click", () => {
    div.remove();
  });
  return div;
}

function populateForm(formData) {
  const form = document.getElementById("config");
  form.days.value = Number.isFinite(formData.days)
    ? Math.max(1, Math.min(formData.days, 7300))
    : 30;
  form.limit.value = Number.isFinite(formData.limit)
    ? Math.max(1, Math.min(formData.limit, 10000))
    : 100;
  form.useFishEyes.checked = formData.useFishEyes === true;
  _selectedFishId = formData.fishId;
  const container = document.getElementById("schedule-rows");
  container.innerHTML = "";
  for (const entry of formData.schedule) {
    container.appendChild(createScheduleRow(entry));
  }
}

function readSchedule() {
  const rows = document.querySelectorAll("#schedule-rows .schedule-row");
  const result = [];
  for (const row of rows) {
    const selects = row.querySelectorAll("select");
    const inputs = row.querySelectorAll("input[type=text]");
    const dayVal = selects[0].value;
    const startSec = timeToSecs(inputs[0].value);
    const endSec = timeToSecs(inputs[1].value);
    if (isNaN(startSec) || isNaN(endSec)) continue;
    const entry = { startSec, endSec };
    if (dayVal !== "") {
      entry.dayOfWeek = parseInt(dayVal, 10);
    }
    result.push(entry);
  }
  return result;
}

function readForm() {
  const form = document.getElementById("config");
  const parsedDays = parseInt(form.days.value, 10);
  const parsedLimit = parseInt(form.limit.value, 10);
  const days = Number.isFinite(parsedDays)
    ? Math.max(1, Math.min(parsedDays, 7300))
    : 7;
  const limit = Number.isFinite(parsedLimit)
    ? Math.max(1, Math.min(parsedLimit, 10000))
    : 100;
  return {
    fishId: _selectedFishId || 0,
    schedule: readSchedule(),
    days,
    limit,
    useFishEyes: form.useFishEyes.checked,
  };
}

function withTimeout(promise, milliseconds) {
  let timeoutId;
  const timeout = new Promise((_, reject) => {
    timeoutId = setTimeout(
      () => reject(new Error(`Search timed out after ${milliseconds / 1000}s`)),
      milliseconds,
    );
  });
  return Promise.race([promise, timeout]).finally(() => {
    clearTimeout(timeoutId);
  });
}

async function query(formData, trigger) {
  const queryGeneration = ++_queryGeneration;
  try {
    const elStatus = document.getElementById("status");
    const elTbody = document.getElementById("tbody");
    const elTable = document.getElementById("results");

    elTbody.innerHTML = "";
    elTable.style.display = "none";

    const nowUnix = Math.floor(Date.now() / 1000);
    const nowEorzea = unix_to_eorzea_esec(BigInt(nowUnix));
    const timeperiodSecs = BigInt(formData.days * 86400);
    const timezoneName = Intl.DateTimeFormat().resolvedOptions().timeZone;
    const limit = formData.limit || 100;

    elStatus.textContent = `Searching the next ${formData.days} day(s) for windows...`;
    // yield so the status message paints before the blocking wasm call
    await new Promise((r) => setTimeout(r, 0));
    if (queryGeneration !== _queryGeneration) return;

    const fishInfo = JSON.parse(await get_fish(formData.fishId));
    if (queryGeneration !== _queryGeneration) return;
    _lastFishName = fishInfo.name;
    document.getElementById("results-fish-name").textContent =
      fishInfo.name + (formData.useFishEyes === true ? " (Fish Eyes)" : "");

    // show fish info
    const infoBar = document.getElementById("fish-info-bar");
    infoBar.style.display = "flex";
    infoBar.innerHTML = "";
    const caught = getList(CAUGHT_KEY);
    const favs = getList(FAV_KEY);
    infoBar.appendChild(
      createFishCard(fishInfo, {
        caught: caught.includes(fishInfo.id),
        faved: favs.includes(fishInfo.id),
        inline: true,
      }),
    );
    updateResultsNote();

    let windowFishId = formData.fishId;
    if (
      isAlwaysUp(fishInfo) &&
      fishInfo.moochPath?.length &&
      fishInfo.baitId != null
    ) {
      try {
        const moochFishInfo = JSON.parse(await get_fish(fishInfo.baitId));
        if (queryGeneration !== _queryGeneration) return;
        if (!isAlwaysUp(moochFishInfo)) {
          windowFishId = moochFishInfo.id;
        }
      } catch (err) {
        if (!String(err).includes("Fish not found")) throw err;
      }
    }

    const alwaysUp =
      isAlwaysUp(fishInfo) &&
      !fishInfo.intuitionRequirements?.length &&
      !fishInfo.moochPath?.length;
    if (alwaysUp) {
      _lastWindows = [];
      elStatus.textContent = "Fish is always up.";
      elTable.style.display = "";
      document.getElementById("download-ics").style.display = "none";
      const row = document.createElement("tr");
      row.innerHTML = '<td colspan="6">always up</td>';
      elTbody.appendChild(row);
      openResults(trigger);
      return;
    }

    const windowsJson = await withTimeout(
      get_fish_windows_in_schedule(
        windowFishId,
        nowEorzea,
        JSON.stringify(formData.schedule),
        timeperiodSecs,
        limit,
        timezoneName,
        FILTER_INTUITION,
        formData.useFishEyes === true,
        true,
      ),
      10000,
    );
    if (queryGeneration !== _queryGeneration) return;
    const windows = JSON.parse(windowsJson);
    _lastWindows = windows;

    elStatus.textContent = `Found ${windows.length} window(s)${windows.length >= limit ? " (limit)" : ""} in the next ${formData.days} day(s).`;
    elTable.style.display = "";
    const dlBtn = document.getElementById("download-ics");
    dlBtn.style.display = windows.length > 0 ? "" : "none";

    if (windows.length === 0) {
      const row = document.createElement("tr");
      row.innerHTML =
        '<td colspan="6">No windows found within the schedule.</td>';
      elTbody.appendChild(row);
      openResults(trigger);
      return;
    }

    const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const fmtDate = (d) => d.toLocaleDateString();
    const fmtTime = (d) => d.toLocaleTimeString();

    let savedCount = 0;
    for (let i = 0; i < windows.length; i++) {
      const w = windows[i];
      const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
      const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
      const startD = new Date(startUnix * 1000);
      const endD = new Date(endUnix * 1000);
      const startDay = days[startD.getDay()];
      const { whenStr } = fmtWhen(startUnix - nowUnix);
      const row = document.createElement("tr");
      const wMeta = {
        ...w,
        fishId: _selectedFishId,
        fishName: _lastFishName,
        intuitionLengthSeconds: fishInfo.intuitionLengthSeconds,
      };
      const wSaved = isWindowSaved(wMeta);
      if (wSaved) savedCount++;
      row.innerHTML = `
  <td>${escapeHtml(i + 1)}${fishEyesIndicator(w)}</td>
  <td>${escapeHtml(whenStr)}</td>
  <td>${escapeHtml(startDay)} ${escapeHtml(fmtDate(startD))}</td>
  <td>${escapeHtml(fmtTime(startD))} - ${escapeHtml(fmtTime(endD))}</td>
  <td>${escapeHtml(eoTime(w.startDisplay))} - ${escapeHtml(eoTime(w.endDisplay))}</td>
  <td>
    <button type="button" class="save-window-btn${wSaved ? " saved" : ""}" data-idx="${i}" title="${wSaved ? "Remove saved window" : "Save window"}" aria-label="${wSaved ? "Remove saved window" : "Save window"}">${wSaved ? "&#128278;" : "&#128204;"}</button>
    <button type="button" class="dl-single" data-idx="${i}" aria-label="Download window as calendar file">&#128197;&#11015;</button>
    ${getGoogleCalendarEnabled() ? `<button type="button" class="dl-gcal" data-idx="${i}" aria-label="Add to Google Calendar">&#128197;<span class="gcal-logo">G</span></button>` : ""}
  </td>
`;
      elTbody.appendChild(row);

      const intuitionInfo = intuitionInfoHtml(
        w,
        "intuition-detail",
        fishInfo.intuitionLengthSeconds,
        true,
      );
      if (intuitionInfo) {
        const detailRow = document.createElement("tr");
        detailRow.className = "intuition-detail-row";
        detailRow.innerHTML = `
          <td colspan="6">
            ${intuitionInfo}
          </td>
        `;
        elTbody.appendChild(detailRow);
      }
    }

    elStatus.textContent =
      `Found ${windows.length} window(s)${windows.length >= limit ? " (limit)" : ""} in the next ${formData.days} day(s).` +
      (savedCount ? ` ${savedCount} saved.` : "");
    updateSaveButtons();
    openResults(trigger);
  } catch (err) {
    if (queryGeneration === _queryGeneration) throw err;
  }
}

function fmtIcsDate(unixSecs) {
  const d = new Date(unixSecs * 1000);
  return d.toISOString().replace(/[-:]/g, "").split(".")[0] + "Z";
}

function icsVevent(w, fishName, alarmMinutes) {
  const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
  const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
  const uid = `${windowKey(w)}@ffxivfishing`;
  const displayName = escapeIcsText((fishName || "Fish") + " Window");
  const lines = [
    "BEGIN:VEVENT",
    "UID:" + uid,
    "DTSTAMP:" + fmtIcsDate(Math.floor(Date.now() / 1000)),
    "DTSTART:" + fmtIcsDate(startUnix),
    "DTEND:" + fmtIcsDate(endUnix),
    "SUMMARY:" + displayName,
  ];
  if (alarmMinutes != null) {
    lines.push(
      "BEGIN:VALARM",
      "ACTION:DISPLAY",
      "DESCRIPTION:" + displayName,
      "TRIGGER:-PT" + alarmMinutes + "M",
      "END:VALARM",
    );
  }
  lines.push("END:VEVENT");
  return lines;
}

function getIcsAlarmMinutes() {
  const settings = getNotificationSettings();
  return settings?.exportAlarm ? settings.minutesBefore : null;
}

function escapeIcsText(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/;/g, "\\;")
    .replace(/,/g, "\\,")
    .replace(/\r?\n/g, "\\n");
}

function downloadIcsFile(lines, filename) {
  const blob = new Blob([lines.join("\r\n") + "\r\n"], {
    type: "text/calendar;charset=utf-8",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function downloadIcsCalendar(vevents, filename, prodid) {
  const lines = [
    "BEGIN:VCALENDAR",
    "VERSION:2.0",
    "PRODID:-" + (prodid || "//ffxivfishing//EN"),
    ...vevents.flat(),
    "END:VCALENDAR",
  ];
  downloadIcsFile(lines, filename);
}

function kebab(s) {
  return s.toLowerCase().replace(/\s+/g, "-");
}

function downloadIcs() {
  if (_lastWindows.length === 0) return;
  downloadIcsCalendar(
    _lastWindows.map((w) => icsVevent(w, _lastFishName, getIcsAlarmMinutes())),
    "fish-schedule-" + kebab(_lastFishName) + ".ics",
    "//fish-schedule-tool//EN",
  );
}

function downloadSingleIcs(idx) {
  const w = _lastWindows[idx];
  if (!w) return;
  downloadIcsCalendar(
    [icsVevent(w, _lastFishName, getIcsAlarmMinutes())],
    "window-" + (idx + 1) + "-" + kebab(_lastFishName) + ".ics",
  );
}

function getGoogleCalendarEnabled() {
  return getNotificationSettings().googleCalendar === true;
}

function fmtGoogleDate(unixSec) {
  return new Date(unixSec * 1000)
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}/, "");
}

function googleCalendarUrl(unixStart, unixEnd, title, description) {
  const params = new URLSearchParams();
  params.set("action", "TEMPLATE");
  params.set("text", title);
  params.set("dates", fmtGoogleDate(unixStart) + "/" + fmtGoogleDate(unixEnd));
  if (description) params.set("details", description);
  return "https://www.google.com/calendar/render?" + params.toString();
}

function downloadSingleGcal(idx) {
  const w = _lastWindows[idx];
  if (!w) return;
  const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
  const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
  globalThis.open(
    googleCalendarUrl(
      startUnix,
      endUnix,
      _lastFishName + " Window",
      "Window " +
      (idx + 1) +
      " for " +
      _lastFishName +
      " ET " +
      eoTime(w.startDisplay) +
      " - " +
      eoTime(w.endDisplay),
    ),
    "_blank",
    "noopener",
  );
}

function downloadSavedGcalItem(w) {
  const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
  const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
  globalThis.open(
    googleCalendarUrl(
      startUnix,
      endUnix,
      w.fishName + " (" + eoTime(w.startDisplay) + ")",
      "Window for " +
      w.fishName +
      "\\nET " +
      eoTime(w.startDisplay) +
      " - " +
      eoTime(w.endDisplay),
    ),
    "_blank",
    "noopener",
  );
}

// ---- Saved Windows ----

function getList(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

function setList(key, list) {
  try {
    localStorage.setItem(key, JSON.stringify(list));
  } catch { }
}

function exportLocalStorage() {
  const data = {};
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key !== null) data[key] = localStorage.getItem(key);
  }
  return JSON.stringify(data, null, 2);
}

function parseLocalStorageImport(raw) {
  let data;
  try {
    data = JSON.parse(raw);
  } catch {
    throw new Error("Import is not valid JSON.");
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    throw new Error("Import must be a JSON object of storage keys.");
  }

  const values = new Map();
  for (const [key, value] of Object.entries(data)) {
    const serialized =
      typeof value === "string" ? value : JSON.stringify(value);
    if (serialized === undefined) {
      throw new Error(`Cannot import value for ${key}.`);
    }
    values.set(key, serialized);
  }
  return values;
}

function applyLocalStorageImport(raw) {
  const values = parseLocalStorageImport(raw);
  const previous = new Map();
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key !== null) previous.set(key, localStorage.getItem(key));
  }

  try {
    localStorage.clear();
    for (const [key, value] of values) {
      localStorage.setItem(key, value);
    }
  } catch (error) {
    localStorage.clear();
    for (const [key, value] of previous) {
      localStorage.setItem(key, value);
    }
    throw new Error(`Import could not be applied: ${error.message}`);
  }

  populateForm(getFormData());
  renderSavedWindows();
  initializeNotificationControls();
  startSavedWindowTimer();
  startNotificationTimer();
  renderFishList();
  updateSaveButtons();
}

const SAVED_WINDOWS_KEY = "savedWindows";
const NOTIFICATION_SETTINGS_KEY = "notificationSettings";
const DEFAULT_NOTIFICATION_MINUTES = 15;
const MAX_NOTIFICATION_MINUTES = 1440;

function isSavedWindow(value) {
  if (!value || typeof value !== "object") return false;
  if (!Number.isInteger(value.fishId) || typeof value.fishName !== "string") {
    return false;
  }
  if (
    typeof value.startDisplay !== "string" ||
    typeof value.endDisplay !== "string"
  ) {
    return false;
  }
  try {
    BigInt(value.startEsec);
    BigInt(value.endEsec);
    return true;
  } catch {
    return false;
  }
}

function getSavedWindows() {
  return getList(SAVED_WINDOWS_KEY).filter(isSavedWindow);
}

function supportsNotifications() {
  return "Notification" in window;
}

function canConfigureNotifications() {
  return supportsNotifications() && Notification.permission !== "denied";
}

function getNotificationSettings() {
  let settings = null;
  try {
    settings = JSON.parse(
      localStorage.getItem(NOTIFICATION_SETTINGS_KEY) || "null",
    );
  } catch { }

  const minutes = Number(settings?.minutesBefore);
  return {
    enabled: settings?.enabled === true,
    exportAlarm: settings?.exportAlarm !== false,
    googleCalendar: settings?.googleCalendar === true,
    minutesBefore: Number.isFinite(minutes)
      ? Math.max(1, Math.min(Math.round(minutes), MAX_NOTIFICATION_MINUTES))
      : DEFAULT_NOTIFICATION_MINUTES,
  };
}

function saveNotificationSettings(settings) {
  try {
    localStorage.setItem(NOTIFICATION_SETTINGS_KEY, JSON.stringify(settings));
  } catch { }
}

function setNotificationStatus(message, type = "") {
  const status = document.getElementById("notification-status");
  if (!status) return;
  status.className = `notification-status${type ? ` ${type}` : ""}`;
  status.textContent = message;
}

function renderNotificationStatus() {
  if (!supportsNotifications()) {
    setNotificationStatus(
      "This browser does not support notifications.",
      "error",
    );
    return;
  }

  const settings = getNotificationSettings();
  if (settings.enabled && Notification.permission === "granted") {
    setNotificationStatus("Notifications set up successfully.", "success");
  } else if (Notification.permission === "denied") {
    setNotificationStatus(
      "Notifications are blocked in browser settings.",
      "error",
    );
  } else {
    setNotificationStatus("Notifications are off.");
  }
}

function updateNotificationInputState() {
  const checkbox = document.getElementById("enable-notifications");
  const minutes = document.getElementById("notification-minutes");
  if (!checkbox || !minutes) return;
  minutes.disabled = !checkbox.checked || !supportsNotifications();
}

function initializeNotificationControls() {
  const settingsPanel = document.getElementById("notification-settings");
  const checkbox = document.getElementById("enable-notifications");
  const minutes = document.getElementById("notification-minutes");
  const icsAlarm = document.getElementById("export-ics-alarm");
  const googleCal = document.getElementById("export-google-calendar");
  if (!settingsPanel || !checkbox || !minutes || !icsAlarm || !googleCal)
    return;

  updateNotificationAvailability();

  const settings = getNotificationSettings();
  if (
    settings.enabled &&
    (!("Notification" in window) || Notification.permission !== "granted")
  ) {
    settings.enabled = false;
    saveNotificationSettings(settings);
  }
  checkbox.checked = settings.enabled;
  minutes.value = settings.minutesBefore;
  icsAlarm.checked = settings.exportAlarm;
  googleCal.checked = settings.googleCalendar;
  updateNotificationInputState();
  renderNotificationStatus();
}

function clearNotificationTimer() {
  if (_notificationTimerId) {
    clearTimeout(_notificationTimerId);
    _notificationTimerId = null;
  }
}

function getWindowTimes(w) {
  try {
    return {
      ...w,
      startUnix: Number(unix_from_eorzea_time(BigInt(w.startEsec))),
      endUnix: Number(unix_from_eorzea_time(BigInt(w.endEsec))),
    };
  } catch {
    return null;
  }
}

function getNextNotificationWindow(minutesBefore) {
  const nowUnix = Date.now() / 1000;
  let next = null;
  for (const saved of getSavedWindows()) {
    const window = getWindowTimes(saved);
    if (!window || window.startUnix <= nowUnix) continue;
    const reminderUnix = window.startUnix - minutesBefore * 60;
    if (reminderUnix <= nowUnix) continue;
    if (!next || reminderUnix < next.reminderUnix) {
      next = { ...window, reminderUnix };
    }
  }
  return next;
}

function formatNotificationWindow(window) {
  const date = new Date(window.startUnix * 1000);
  return `${date.toLocaleDateString()} ${date.toLocaleTimeString()} (ET ${eoTime(window.startDisplay)})`;
}

function showWindowNotification(windowInfo, minutesBefore) {
  if (!supportsNotifications() || Notification.permission !== "granted") {
    return false;
  }

  const title = `${windowInfo.fishName} window is coming up`;
  const body = `Starts in ${minutesBefore} minute${minutesBefore === 1 ? "" : "s"} at ${formatNotificationWindow(windowInfo)}.`;
  try {
    const notification = new Notification(title, {
      body,
      icon: "./icon-192.png",
      badge: "./icon-192.png",
      tag: `window-${windowKey(windowInfo)}`,
    });
    notification.onclick = () => {
      globalThis.window.focus();
      notification.close();
    };
    return true;
  } catch {
    setNotificationStatus(
      "The browser could not display a notification.",
      "error",
    );
    return false;
  }
}

function startNotificationTimer() {
  clearNotificationTimer();
  if (!canConfigureNotifications() || !_wasmReady) return;

  const settings = getNotificationSettings();
  if (!settings.enabled || Notification.permission !== "granted") {
    renderNotificationStatus();
    return;
  }

  const next = getNextNotificationWindow(settings.minutesBefore);
  if (!next) {
    renderNotificationStatus();
    return;
  }

  const delay = Math.max(1000, next.reminderUnix * 1000 - Date.now());
  _notificationTimerId = setTimeout(
    () => {
      _notificationTimerId = null;
      if (Date.now() / 1000 >= next.reminderUnix) {
        const current = getSavedWindows().find(
          (saved) => windowKey(saved) === windowKey(next),
        );
        const currentWindow = current && getWindowTimes(current);
        if (currentWindow) {
          showWindowNotification(currentWindow, settings.minutesBefore);
        }
      }
      startNotificationTimer();
    },
    Math.min(delay, 2147483647),
  );
  renderNotificationStatus();
}

async function handleNotificationToggle() {
  const checkbox = document.getElementById("enable-notifications");
  const minutesInput = document.getElementById("notification-minutes");
  if (!checkbox || !minutesInput) return;

  const settings = getNotificationSettings();
  if (!checkbox.checked) {
    settings.enabled = false;
    saveNotificationSettings(settings);
    updateNotificationInputState();
    clearNotificationTimer();
    renderNotificationStatus();
    return;
  }

  checkbox.disabled = true;
  try {
    if (!("Notification" in window)) {
      throw new Error("This browser does not support notifications.");
    }
    let permission = Notification.permission;
    if (permission === "default") {
      permission = await Notification.requestPermission();
    }
    if (permission !== "granted") {
      checkbox.checked = false;
      settings.enabled = false;
      saveNotificationSettings(settings);
      renderNotificationStatus();
      return;
    }

    const inputMinutes = Number(minutesInput.value);
    settings.minutesBefore = Number.isFinite(inputMinutes)
      ? Math.max(
        1,
        Math.min(Math.round(inputMinutes), MAX_NOTIFICATION_MINUTES),
      )
      : DEFAULT_NOTIFICATION_MINUTES;
    minutesInput.value = settings.minutesBefore;
    settings.enabled = true;
    saveNotificationSettings(settings);
    renderNotificationStatus();
    startNotificationTimer();
  } catch (error) {
    checkbox.checked = false;
    settings.enabled = false;
    saveNotificationSettings(settings);
    setNotificationStatus(error.message, "error");
  } finally {
    checkbox.disabled = false;
    updateNotificationInputState();
    updateNotificationAvailability();
  }
}

function updateNotificationMinutes() {
  const input = document.getElementById("notification-minutes");
  if (!input) return;
  const value = Number(input.value);
  const minutesBefore = Number.isFinite(value)
    ? Math.max(1, Math.min(Math.round(value), MAX_NOTIFICATION_MINUTES))
    : DEFAULT_NOTIFICATION_MINUTES;
  input.value = minutesBefore;
  const settings = getNotificationSettings();
  settings.minutesBefore = minutesBefore;
  saveNotificationSettings(settings);
  startNotificationTimer();
}

function updateNotificationAvailability() {
  const checkbox = document.getElementById("enable-notifications");
  if (!checkbox) return;
  const supported = supportsNotifications();
  checkbox.disabled = !supported;
  if (!supported) clearNotificationTimer();
}

function handleIcsAlarmToggle() {
  const checkbox = document.getElementById("export-ics-alarm");
  if (!checkbox) return;
  const settings = getNotificationSettings();
  settings.exportAlarm = checkbox.checked;
  saveNotificationSettings(settings);
}

function handleGoogleCalendarToggle() {
  const checkbox = document.getElementById("export-google-calendar");
  if (!checkbox) return;
  const settings = getNotificationSettings();
  settings.googleCalendar = checkbox.checked;
  saveNotificationSettings(settings);
}

function windowKey(w) {
  return `${w.fishId}-${w.startEsec}-${w.endEsec}`;
}

function getIntuitionMonitorRange(w) {
  const prereqs = w.intuition?.prerequisiteWindows;
  if (!Array.isArray(prereqs) || prereqs.length === 0) return null;
  let start = null;
  for (const p of prereqs) {
    if (isPrerequisiteAlwaysUp(p)) continue;
    const s = Number(unix_from_eorzea_time(BigInt(p.startEsec)));
    if (start === null || s < start) start = s;
  }
  if (start === null) return null;
  const end = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
  return { start, end };
}

function fmtPrereqStatus(startUnix, endUnix, nowUnix) {
  const { displayWhen, state } = fmtSavedWhen(startUnix, endUnix, nowUnix);
  return { label: displayWhen, state: state === "ongoing" ? "up" : state };
}

function updatePrereqStatus(el, nowUnix) {
  const start = Number(el.dataset.prereqStart);
  const end = Number(el.dataset.prereqEnd);
  const { label, state } = fmtPrereqStatus(start, end, nowUnix);
  el.textContent = label;
  el.classList.remove("up", "past", "future");
  el.classList.add(state);
}

function updateSavedWindowPrereqStatuses(nowUnix) {
  document
    .querySelectorAll("#saved-windows-list .intuition-prerequisite-status")
    .forEach((el) => updatePrereqStatus(el, nowUnix));
}

function isWindowSaved(w) {
  const saved = getSavedWindows();
  const key = windowKey(w);
  return saved.some((s) => windowKey(s) === key);
}

function saveWindow(w) {
  const saved = getSavedWindows();
  const key = windowKey(w);
  if (saved.some((s) => windowKey(s) === key)) return;
  saved.push(w);
  setList(SAVED_WINDOWS_KEY, saved);
  renderSavedWindows();
  startSavedWindowTimer();
  startNotificationTimer();
  updateSaveButtons();
  renderFishList();
}

function removeSavedWindow(w) {
  let saved = getSavedWindows();
  const key = windowKey(w);
  saved = saved.filter((s) => windowKey(s) !== key);
  setList(SAVED_WINDOWS_KEY, saved);
  renderSavedWindows();
  startSavedWindowTimer();
  startNotificationTimer();
  updateSaveButtons();
  renderFishList();
}

function downloadSavedIcs() {
  const saved = getSavedWindows();
  if (saved.length === 0) return;
  downloadIcsCalendar(
    saved.map((w) => icsVevent(w, w.fishName, getIcsAlarmMinutes())),
    "saved-windows.ics",
    "//saved-windows//EN",
  );
}

function downloadSavedIcsItem(w) {
  downloadIcsCalendar(
    [icsVevent(w, w.fishName, getIcsAlarmMinutes())],
    "window-" + kebab(w.fishName) + ".ics",
  );
}

function intuitionInfoHtml(
  w,
  containerClass,
  intuitionLengthSeconds,
  showStatus,
) {
  const prerequisiteWindows = w.intuition?.prerequisiteWindows;
  if (!prerequisiteWindows?.length) return "";
  const intuitionLength = formatRealDuration(intuitionLengthSeconds);
  const title = `Intuition${intuitionLength ? ` (${intuitionLength})` : ""}`;

  const prerequisiteLines = prerequisiteWindows
    .map((prerequisite) => {
      const prerequisiteStartUnix = Number(
        unix_from_eorzea_time(BigInt(prerequisite.startEsec)),
      );
      const prerequisiteEndUnix = Number(
        unix_from_eorzea_time(BigInt(prerequisite.endEsec)),
      );
      const prerequisiteStart = new Date(prerequisiteStartUnix * 1000);
      const prerequisiteEnd = new Date(prerequisiteEndUnix * 1000);
      const fishName = prerequisite.fish || `Fish #${prerequisite.fishId}`;
      const eorzeaStart = eoTime(prerequisite.startDisplay);
      const eorzeaEnd = eoTime(prerequisite.endDisplay);
      const alwaysUp = eorzeaStart === "00:00" && eorzeaEnd === "00:00";
      const localWindow = alwaysUp
        ? "always up"
        : formatLocalWindow(prerequisiteStart, prerequisiteEnd);
      const eorzeaWindow = alwaysUp
        ? "always up"
        : `${eorzeaStart} - ${eorzeaEnd}`;
      const timeWindow = alwaysUp
        ? `<span class="intuition-prerequisite-local">${localWindow}</span>`
        : `<span class="intuition-prerequisite-et">ET ${escapeHtml(eorzeaWindow)}</span>
          <span class="intuition-prerequisite-separator"> | </span>
          <span class="intuition-prerequisite-local">${escapeHtml(localWindow)}</span>`;
      let statusHtml = "";
      if (showStatus && !alwaysUp) {
        const { label, state } = fmtPrereqStatus(
          prerequisiteStartUnix,
          prerequisiteEndUnix,
          Math.floor(Date.now() / 1000),
        );
        statusHtml = `<span class="intuition-prerequisite-status ${state}" data-prereq-start="${prerequisiteStartUnix}" data-prereq-end="${prerequisiteEndUnix}">${escapeHtml(label)}</span>`;
      }
      return `
        <li>
          <span class="intuition-prerequisite-fish">${escapeHtml(`${prerequisite.amount} ${fishName}`)}${fishEyesIndicator(prerequisite)}${statusHtml}</span>
          <span class="intuition-prerequisite-window">${timeWindow}</span>
        </li>`;
    })
    .join("");

  return `
    <div class="${containerClass}">
      <strong>${escapeHtml(title)}</strong>
      <ol>${prerequisiteLines}</ol>
    </div>`;
}

function renderSavedWindows() {
  const saved = getSavedWindows().sort((a, b) => {
    const aSec = BigInt(a.startEsec);
    const bSec = BigInt(b.startEsec);
    if (aSec < bSec) return -1;
    if (aSec > bSec) return 1;
    return 0;
  });
  const container = document.getElementById("saved-windows-list");
  const empty = document.getElementById("saved-windows-empty");

  container.innerHTML = "";

  const dlBtn = document.getElementById("download-saved-ics");

  if (saved.length === 0) {
    empty.style.display = "";
    document.getElementById("saved-count").textContent = "";
    dlBtn.style.display = "none";
    return;
  }

  empty.style.display = "none";
  document.getElementById("saved-count").textContent = `(${saved.length})`;
  dlBtn.style.display = "";

  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  for (const w of saved) {
    const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
    const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
    const startD = new Date(startUnix * 1000);
    const startDay = days[startD.getDay()];
    const dateStr = startD.toLocaleDateString();
    const timeStr = `${startD.toLocaleTimeString()} - ${new Date(endUnix * 1000).toLocaleTimeString()}`;
    const eoStr = `${eoTime(w.startDisplay)} - ${eoTime(w.endDisplay)}`;

    const nowUnix = Math.floor(Date.now() / 1000);
    const { displayWhen, state } = fmtSavedWhen(startUnix, endUnix, nowUnix);
    const hasNote = getNote(w.fishId).length > 0;

    const div = document.createElement("div");
    div.className = `saved-item ${state}`;
    div.dataset.start = startUnix;
    div.dataset.end = endUnix;
    div.dataset.fishId = w.fishId;
    div.dataset.fishName = w.fishName;
    div.dataset.state = state;
    updateSavedWindowProgress(div, nowUnix, state);
    div.innerHTML = `
      <div class="saved-body">
        <div class="saved-top-row">
          <span class="fish-name truncate" tabindex="0" role="button" aria-label="Show schedule for ${escapeHtml(w.fishName)}">${escapeHtml(w.fishName)}${hasNote ? `<span class="saved-note-indicator" title="Has note">&#128221;</span>` : ""}${fishEyesIndicator(w)}</span>
          <span class="window-when">${escapeHtml(displayWhen)}</span>
          <span class="saved-actions">
            <button type="button" class="dl-single" title="Download .ics" aria-label="Download saved window as calendar file">&#128197;&#11015;</button>
            ${getGoogleCalendarEnabled() ? `<button type="button" class="dl-gcal" title="Add to Google Calendar" aria-label="Add saved window to Google Calendar">&#128197;<span class="gcal-logo">G</span></button>` : ""}
            <button type="button" class="remove-saved" title="Remove" aria-label="Remove saved window">&#10005;</button>
          </span>
        </div>
        <span class="window-detail">${escapeHtml(startDay)} | ${escapeHtml(dateStr)} ${escapeHtml(timeStr)} | ET ${escapeHtml(eoStr)}</span>
        ${intuitionInfoHtml(
      w,
      "saved-intuition-setup",
      w.intuitionLengthSeconds ??
      _allFishInfo.find((fish) => fish.id === w.fishId)
        ?.intuitionLengthSeconds,
      true,
    )}
      </div>
    `;
    const savedFish = div.querySelector(".fish-name");
    const selectSavedFish = () => {
      setSelectedFish(w.fishId);
      const p = readForm();
      if (p.schedule.length === 0)
        p.schedule.push({ startSec: 0, endSec: 86400 });
      saveSearch(p);
      const elStatus = document.getElementById("status");
      elStatus.className = "status";
      query(p, savedFish).catch((err) => {
        elStatus.className = "status error";
        elStatus.textContent = `Error: ${err}`;
      });
    };
    savedFish.addEventListener("click", selectSavedFish);
    savedFish.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectSavedFish();
      }
    });
    div.querySelector(".remove-saved").addEventListener("click", () => {
      removeSavedWindow(w);
    });
    div.querySelector(".dl-single").addEventListener("click", () => {
      downloadSavedIcsItem(w);
    });
    const gcalBtn = div.querySelector(".dl-gcal");
    if (gcalBtn) {
      gcalBtn.addEventListener("click", () => {
        downloadSavedGcalItem(w);
      });
    }
    container.appendChild(div);
  }
  updateSavedWindowPrereqStatuses(Math.floor(Date.now() / 1000));
}

let _savedTimerId = null;
let _fishListTimerId = null;

function updateSavedWindowProgress(div, nowUnix, state) {
  const startUnix = parseInt(div.dataset.start, 10);
  const endUnix = parseInt(div.dataset.end, 10);
  const progress =
    state === "ongoing" && endUnix > startUnix
      ? Math.min(
        100,
        Math.max(0, ((nowUnix - startUnix) / (endUnix - startUnix)) * 100),
      )
      : 0;
  div.style.setProperty("--window-progress", `${progress}%`);
}

function updateSavedWindowTimers() {
  const nowUnix = Math.floor(Date.now() / 1000);
  const announcements = [];
  document
    .querySelectorAll("#saved-windows-list .saved-item")
    .forEach((div) => {
      const startUnix = parseInt(div.dataset.start, 10);
      const endUnix = parseInt(div.dataset.end, 10);
      if (isNaN(startUnix)) return;
      const { displayWhen, state } = fmtSavedWhen(startUnix, endUnix, nowUnix);
      const previousState = div.dataset.state;
      div.querySelector(".window-when").textContent = displayWhen;
      div.className = `saved-item ${state}`;
      div.dataset.state = state;
      if (previousState && previousState !== state) {
        const fishName = div.dataset.fishName || "Saved fish";
        announcements.push(
          state === "ongoing"
            ? `${fishName} window is active, ${displayWhen}.`
            : `${fishName} window ended.`,
        );
      }
      updateSavedWindowProgress(div, nowUnix, state);
    });
  updateSavedWindowPrereqStatuses(nowUnix);
  if (announcements.length) {
    document.getElementById("saved-window-announcer").textContent =
      announcements.join(" ");
  }
}

const INTUITION_UPDATE_INTERVAL_MS = 5000;

function startSavedWindowTimer() {
  if (_savedTimerId) {
    clearInterval(_savedTimerId);
    _savedTimerId = null;
  }

  const saved = getSavedWindows();
  const nowUnix = Math.floor(Date.now() / 1000);
  let hasActiveWindow = false;
  let hasMonitorWindow = false;
  let nextStart = Infinity;
  for (const w of saved) {
    const startUnix = Number(unix_from_eorzea_time(BigInt(w.startEsec)));
    const endUnix = Number(unix_from_eorzea_time(BigInt(w.endEsec)));
    if (startUnix <= nowUnix && endUnix > nowUnix) {
      hasActiveWindow = true;
    } else if (
      startUnix > nowUnix &&
      endUnix > nowUnix &&
      startUnix < nextStart
    ) {
      nextStart = startUnix;
    }

    const monitor = getIntuitionMonitorRange(w);
    if (monitor) {
      if (nowUnix >= monitor.start && nowUnix < monitor.end) {
        hasMonitorWindow = true;
      }
      if (monitor.start > nowUnix && monitor.start < nextStart) {
        nextStart = monitor.start;
      }
    }
  }

  if (!hasActiveWindow && !hasMonitorWindow && nextStart === Infinity) return;

  const diff = nextStart - nowUnix;
  const interval = hasActiveWindow
    ? 1000
    : hasMonitorWindow
      ? INTUITION_UPDATE_INTERVAL_MS
      : diff <= 120
        ? 1000
        : diff > 3600
          ? 900000
          : Math.min(60000, Math.max(1000, diff * 1000));

  _savedTimerId = setInterval(() => {
    updateSavedWindowTimers();
    clearInterval(_savedTimerId);
    startSavedWindowTimer();
  }, interval);

  updateSavedWindowTimers();
}

function startFishListTimer() {
  if (_fishListTimerId) {
    clearInterval(_fishListTimerId);
  }
  _fishListTimerId = setInterval(renderFishList, 15 * 60 * 1000);
}

function updateSaveButtons() {
  document.querySelectorAll(".save-window-btn").forEach((btn) => {
    const idx = parseInt(btn.dataset.idx, 10);
    const w = _lastWindows[idx];
    if (!w) return;
    const tr = btn.closest("tr");
    const savedFlag = isWindowSaved({
      ...w,
      fishId: _selectedFishId,
      fishName: _lastFishName,
    });
    btn.classList.toggle("saved", savedFlag);
    btn.title = savedFlag ? "Remove saved window" : "Save window";
    btn.setAttribute(
      "aria-label",
      savedFlag ? "Remove saved window" : "Save window",
    );
    btn.innerHTML = savedFlag ? "&#10060;" : "&#128204;";
    tr.classList.toggle("saved-window", savedFlag);
  });
}

// ---- Favourite Fish ----

const FAV_KEY = "favFish";

function toggleFavFish(id) {
  let list = getList(FAV_KEY);
  const idx = list.indexOf(id);
  if (idx >= 0) {
    list.splice(idx, 1);
  } else {
    list.push(id);
  }
  setList(FAV_KEY, list);
  renderFishList();
  const isFaved = list.includes(id);
  document.querySelectorAll(".fav-star").forEach((button) => {
    if (button.dataset.fishId !== String(id)) return;
    button.classList.toggle("faved", isFaved);
    button.setAttribute(
      "aria-label",
      isFaved ? "Remove fish from favourites" : "Add fish to favourites",
    );
    button.innerHTML = isFaved ? "&#9829;" : "&#9825;";
  });
}

function setSelectedFish(id) {
  _selectedFishId = id;
}

// ---- Fish Notes ----

const NOTES_KEY = "fishNotes";

function getFishNotes() {
  try {
    const raw = localStorage.getItem(NOTES_KEY);
    if (!raw) return {};
    const obj = JSON.parse(raw);
    return obj && typeof obj === "object" && !Array.isArray(obj) ? obj : {};
  } catch {
    return {};
  }
}

function getNote(id) {
  const notes = getFishNotes();
  const value = notes[String(id)];
  return typeof value === "string" ? value : "";
}

function setNote(id, text) {
  const notes = getFishNotes();
  const key = String(id);
  text = String(text ?? "").trim();
  if (text === "") {
    delete notes[key];
  } else {
    notes[key] = text;
  }
  try {
    localStorage.setItem(NOTES_KEY, JSON.stringify(notes));
  } catch { }
}

let _noteFishId = 0;
let _noteTrigger = null;

function openNoteModal(fishId, fishName, trigger) {
  const panel = document.getElementById("note-modal");
  const overlay = document.getElementById("note-overlay");
  _noteFishId = fishId;
  _noteTrigger = trigger || null;
  document.getElementById("note-fish-name").textContent = fishName;
  const input = document.getElementById("fish-note-input");
  input.value = getNote(fishId);
  document.getElementById("note-status").textContent = "";
  panel.classList.add("open");
  panel.setAttribute("aria-hidden", "false");
  overlay.classList.add("open");
  overlay.setAttribute("aria-hidden", "false");
  updateModalBackgroundInert();
  input.focus();
}

function closeNoteModal() {
  const panel = document.getElementById("note-modal");
  const overlay = document.getElementById("note-overlay");
  panel.classList.remove("open");
  panel.setAttribute("aria-hidden", "true");
  overlay.classList.remove("open");
  overlay.setAttribute("aria-hidden", "true");
  updateModalBackgroundInert();
  if (_noteTrigger && typeof _noteTrigger.focus === "function") {
    _noteTrigger.focus();
  }
  _noteTrigger = null;
}

function updateResultsNote() {
  const el = document.getElementById("results-note-body");
  if (!el) return;
  const note = getNote(_selectedFishId);
  el.textContent = note ? note : "no note";
}

function renderFishNote(id) {
  document
    .querySelectorAll(`.fish-note-indicator[data-fish-id="${id}"]`)
    .forEach((el) => {
      el.style.display = getNote(id) ? "" : "none";
    });
}

function saveNoteFromModal() {
  const text = document.getElementById("fish-note-input").value;
  setNote(_noteFishId, text);
  const status = document.getElementById("note-status");
  status.className = "import-export-status success";
  status.textContent = "Note saved.";
  renderFishNote(_noteFishId);
  renderFishList();
  renderSavedWindows();
  if (_noteFishId === _selectedFishId) updateResultsNote();
}

// ---- Results Panel ----

let _resultsTrigger = null;

function updateModalBackgroundInert() {
  const modalOpen =
    document.getElementById("results-panel").classList.contains("open") ||
    document.getElementById("import-export-modal").classList.contains("open") ||
    document.getElementById("note-modal").classList.contains("open");
  document
    .querySelectorAll(
      "main > *:not(#results-overlay):not(#results-panel):not(#import-export-overlay):not(#import-export-modal):not(#note-overlay):not(#note-modal), footer",
    )
    .forEach((element) => {
      element.inert = modalOpen;
    });
}

function openResults(trigger) {
  const panel = document.getElementById("results-panel");
  const overlay = document.getElementById("results-overlay");
  if (trigger) _resultsTrigger = trigger;
  panel.classList.add("open");
  panel.setAttribute("aria-hidden", "false");
  overlay.classList.add("open");
  overlay.setAttribute("aria-hidden", "false");
  updateModalBackgroundInert();
  panel.focus();
}

function closeResults() {
  const panel = document.getElementById("results-panel");
  const overlay = document.getElementById("results-overlay");
  panel.classList.remove("open");
  panel.setAttribute("aria-hidden", "true");
  overlay.classList.remove("open");
  overlay.setAttribute("aria-hidden", "true");
  updateModalBackgroundInert();
  if (_resultsTrigger && typeof _resultsTrigger.focus === "function") {
    _resultsTrigger.focus();
  }
  _resultsTrigger = null;
}

let _importExportTrigger = null;

function openImportExport(trigger) {
  const panel = document.getElementById("import-export-modal");
  const overlay = document.getElementById("import-export-overlay");
  if (trigger) _importExportTrigger = trigger;
  document.getElementById("storage-export").value = exportLocalStorage();
  document.getElementById("import-export-status").textContent = "";
  panel.classList.add("open");
  panel.setAttribute("aria-hidden", "false");
  overlay.classList.add("open");
  overlay.setAttribute("aria-hidden", "false");
  updateModalBackgroundInert();
  panel.focus();
}

function closeImportExport() {
  const panel = document.getElementById("import-export-modal");
  const overlay = document.getElementById("import-export-overlay");
  panel.classList.remove("open");
  panel.setAttribute("aria-hidden", "true");
  overlay.classList.remove("open");
  overlay.setAttribute("aria-hidden", "true");
  updateModalBackgroundInert();
  if (
    _importExportTrigger &&
    typeof _importExportTrigger.focus === "function"
  ) {
    _importExportTrigger.focus();
  }
  _importExportTrigger = null;
}

document
  .getElementById("results-close")
  .addEventListener("click", closeResults);
document
  .getElementById("results-overlay")
  .addEventListener("click", closeResults);
document
  .getElementById("import-export-open")
  .addEventListener("click", (event) => {
    openImportExport(event.currentTarget);
  });
document
  .getElementById("import-export-close")
  .addEventListener("click", closeImportExport);
document
  .getElementById("import-export-overlay")
  .addEventListener("click", closeImportExport);
document.getElementById("note-close").addEventListener("click", closeNoteModal);
document
  .getElementById("note-overlay")
  .addEventListener("click", closeNoteModal);
document.getElementById("note-save").addEventListener("click", () => {
  saveNoteFromModal();
  closeNoteModal();
});
document.getElementById("note-clear").addEventListener("click", () => {
  document.getElementById("fish-note-input").value = "";
  saveNoteFromModal();
  closeNoteModal();
});
document.getElementById("results-note-edit").addEventListener("click", (e) => {
  openNoteModal(_selectedFishId, _lastFishName, e.currentTarget);
});
document
  .getElementById("storage-import-apply")
  .addEventListener("click", () => {
    const status = document.getElementById("import-export-status");
    status.className = "import-export-status";
    try {
      applyLocalStorageImport(document.getElementById("storage-import").value);
      document.getElementById("storage-export").value = exportLocalStorage();
      status.className = "import-export-status success";
      status.textContent = "Imported data successfully.";
    } catch (error) {
      status.className = "import-export-status error";
      status.textContent = error.message;
    }
  });
document.addEventListener("keydown", (event) => {
  const resultsPanel = document.getElementById("results-panel");
  const importExportPanel = document.getElementById("import-export-modal");
  const notePanel = document.getElementById("note-modal");
  if (event.key === "Escape") {
    if (notePanel.classList.contains("open")) {
      closeNoteModal();
      return;
    }
    if (importExportPanel.classList.contains("open")) {
      closeImportExport();
      return;
    }
    if (resultsPanel.classList.contains("open")) {
      closeResults();
      return;
    }
  }
  const panel = importExportPanel.classList.contains("open")
    ? importExportPanel
    : resultsPanel;
  if (notePanel.classList.contains("open")) {
    if (event.key !== "Tab" || !notePanel.classList.contains("open")) return;
  }
  const activePanel = notePanel.classList.contains("open") ? notePanel : panel;
  if (event.key !== "Tab" || !activePanel.classList.contains("open")) return;
  const focusable = activePanel.querySelectorAll(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
  );
  if (focusable.length === 0) {
    event.preventDefault();
    activePanel.focus();
    return;
  }
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (
    document.activeElement === activePanel ||
    !activePanel.contains(document.activeElement)
  ) {
    event.preventDefault();
    (event.shiftKey ? last : first).focus();
    return;
  }
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
});

async function main() {
  const formData = getFormData();
  populateForm(formData);

  const elStatus = document.getElementById("status");
  try {
    elStatus.textContent = "Initializing WASM...";
    await init();
    await init_default();
    _wasmReady = true;
    renderSavedWindows();
    startSavedWindowTimer();
    startNotificationTimer();
    // preload fish list data
    try {
      const raw = await list_all_fish_info();
      _allFishInfo = JSON.parse(raw);
      renderSavedWindows();
    } catch { }
    renderFishList();
    startFishListTimer();
  } catch (err) {
    elStatus.className = "status error";
    elStatus.textContent = `Error: ${err}`;
  }
}

document.getElementById("add-schedule").addEventListener("click", () => {
  const container = document.getElementById("schedule-rows");
  container.appendChild(createScheduleRow({ startSec: 0, endSec: 86400 }));
});

document.getElementById("apply-btn").addEventListener("click", () => {
  const p = readForm();
  if (p.schedule.length === 0) {
    p.schedule.push({ startSec: 0, endSec: 86400 });
  }
  saveSearch(p);
  _nextWindowCache.clear();
  renderFishList();
});

document
  .getElementById("enable-notifications")
  .addEventListener("change", handleNotificationToggle);
document
  .getElementById("notification-minutes")
  .addEventListener("change", updateNotificationMinutes);
document
  .getElementById("export-ics-alarm")
  .addEventListener("change", handleIcsAlarmToggle);
document
  .getElementById("export-google-calendar")
  .addEventListener("change", handleGoogleCalendarToggle);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    updateNotificationAvailability();
    startNotificationTimer();
  }
});

document.getElementById("config-toggle").addEventListener("click", function () {
  const content = document.getElementById("config-content");
  content.classList.toggle("hidden");
  this.classList.toggle("collapsed");
  this.setAttribute(
    "aria-expanded",
    String(!content.classList.contains("hidden")),
  );
  content.setAttribute(
    "aria-hidden",
    String(content.classList.contains("hidden")),
  );
});

document.getElementById("download-ics").addEventListener("click", downloadIcs);

document
  .getElementById("download-saved-ics")
  .addEventListener("click", downloadSavedIcs);

document.getElementById("saved-toggle").addEventListener("click", function () {
  _savedOpen = !_savedOpen;
  document.getElementById("saved-content").style.display = _savedOpen
    ? ""
    : "none";
  this.classList.toggle("collapsed", !_savedOpen);
  this.setAttribute("aria-expanded", String(_savedOpen));
});

document.getElementById("results").addEventListener("click", (e) => {
  const dlBtn = e.target.closest(".dl-single");
  if (dlBtn) downloadSingleIcs(parseInt(dlBtn.dataset.idx, 10));
  const gcalBtn = e.target.closest(".dl-gcal");
  if (gcalBtn) downloadSingleGcal(parseInt(gcalBtn.dataset.idx, 10));
  const saveBtn = e.target.closest(".save-window-btn");
  if (saveBtn) {
    const idx = parseInt(saveBtn.dataset.idx, 10);
    const w = _lastWindows[idx];
    if (w) {
      const wWithMeta = {
        ...w,
        fishId: _selectedFishId,
        fishName: _lastFishName,
      };
      if (isWindowSaved(wWithMeta)) {
        removeSavedWindow(wWithMeta);
      } else {
        saveWindow(wWithMeta);
      }
    }
  }
});

// ---- Fish List (inline) ----

const CAUGHT_KEY = "caughtFish";

function toggleCaught(id) {
  let list = getList(CAUGHT_KEY);
  const idx = list.indexOf(id);
  if (idx >= 0) list.splice(idx, 1);
  else list.push(id);
  setList(CAUGHT_KEY, list);
  renderFishList();
}

const SPOT_LINK_TARGETS = [
  {
    name: "FishStats",
    domain: "lodinn.github.io",
    icon: "https://lodinn.github.io/favicon.ico",
    getUrl: (spot) =>
      `https://lodinn.github.io/biterates?spot=${encodeURIComponent(spot.name)}`,
  },
  {
    name: "Teamcraft",
    domain: "ffxivteamcraft.com",
    icon: "https://ffxivteamcraft.com/favicon.ico",
    getUrl: (spot) =>
      `https://ffxivteamcraft.com/db/en/fishing-spot/${spot.id}/${kebab(spot.name)}`,
  },
];

let _spotPopup = null;
let _spotPopupTrigger = null;

function ensureSpotPopup() {
  if (_spotPopup) return;
  _spotPopup = document.createElement("div");
  _spotPopup.className = "spot-popup";
  _spotPopup.setAttribute("role", "menu");
  _spotPopup.style.display = "none";
  document.body.appendChild(_spotPopup);
}

function showSpotPopup(spot, triggerEl) {
  ensureSpotPopup();
  _spotPopupTrigger = triggerEl;
  _spotPopup.innerHTML = SPOT_LINK_TARGETS.map((t) => {
    return `<button type="button" class="spot-popup-item" role="menuitem" data-url="${escapeHtml(t.getUrl(spot))}">
      <img src="${escapeHtml(t.icon)}" alt="" width="16" height="16" class="spot-popup-icon" />
      <span class="spot-popup-name">${escapeHtml(t.name)}</span>
      <span class="spot-popup-domain">${escapeHtml(t.domain)}</span>
    </button>`;
  }).join("");
  _spotPopup.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", () => {
      globalThis.open(btn.dataset.url, "_blank", "noopener");
      closeSpotPopup();
    });
  });
  const rect = triggerEl.getBoundingClientRect();
  _spotPopup.style.top = `${rect.bottom + window.scrollY + 4}px`;
  _spotPopup.style.left = `${rect.left + window.scrollX}px`;
  _spotPopup.style.display = "block";
}

function closeSpotPopup() {
  if (_spotPopup) {
    _spotPopup.style.display = "none";
  }
  _spotPopupTrigger = null;
}

function createFishCard(f, opts = {}) {
  const div = document.createElement("div");
  div.className = "fish-item" + (opts.caught ? " caught" : "");
  if (opts.inline)
    div.style.cssText =
      "flex:0 0 100%;border:none;padding:0.2rem 0;background:transparent;";

  const nowUnix = Math.floor(Date.now() / 1000);
  const nowEorzea = unix_to_eorzea_esec(BigInt(nowUnix));
  let nextStr = "";
  const hasConditionalAvailability =
    f.intuitionRequirements?.length || f.moochPath?.length;
  const alwaysUp = isAlwaysUp(f) && !hasConditionalAvailability;
  if (alwaysUp) {
    nextStr = '<span class="fish-next-window">always up</span>';
  } else if (opts.deferNextWindow) {
    nextStr = `<span class="fish-next-window fish-next-deferred" data-fish-id="${f.id}">...</span>`;
  } else {
    const next = getNextFishWindow(f.id, nowUnix, nowEorzea);
    if (next) {
      const startUnix = Number(unix_from_eorzea_time(BigInt(next.startEsec)));
      const endUnix = Number(unix_from_eorzea_time(BigInt(next.endEsec)));
      const { whenStr, state } = fmtWhen(
        startUnix - nowUnix,
        endUnix - nowUnix,
      );
      nextStr = `<span class="fish-next-window ${state}">${state === "future" ? "in " + whenStr : whenStr}</span>${fishEyesIndicator(next)}`;
    }
  }

  const weatherStr =
    (f.previousWeatherSet && f.previousWeatherSet.length
      ? f.previousWeatherSet.join(" / ") + " → "
      : "") + (f.weatherSet ? f.weatherSet.join(" / ") : "") || "-";
  const pct = f.fishUptime !== undefined ? f.fishUptime * 100 : undefined;
  const uptimeStr =
    pct !== undefined
      ? " | " + pct.toFixed(pct < 0.1 ? 2 : 1) + "% uptime"
      : "";
  const requirementLines = [];
  if (opts.inline) {
    if (f.bait) {
      requirementLines.push(`Bait: ${f.bait}`);
    }
    if (Array.isArray(f.moochPath) && f.moochPath.length) {
      requirementLines.push(`Mooch: ${f.moochPath.join(" -> ")}`);
    }
    if (
      Array.isArray(f.intuitionRequirements) &&
      f.intuitionRequirements.length
    ) {
      const intuitionLength = formatRealDuration(f.intuitionLengthSeconds);
      requirementLines.push(
        `Intuition${intuitionLength ? ` (${intuitionLength})` : ""}: ` +
        f.intuitionRequirements
          .map((requirement) => `${requirement.amount} ${requirement.fish}`)
          .join(" | "),
      );
    }
    const technique = [`Tug: ${f.tug}`, `Hookset: ${f.hookset}`];
    if (f.fishEyes) technique.push("Fish Eyes");
    if (f.snagging) technique.push("Snagging");
    if (f.lure && f.lureProc) {
      technique.push(`Lure: ${f.lure === "Ambitious" ? "A" : "M"}`);
    }
    requirementLines.push(technique.join(" | "));
  }
  const requirementsHtml = requirementLines
    .map((line) => `<span class="fish-requirement">${escapeHtml(line)}</span>`)
    .join("");
  const isFaved = opts.faved || false;
  const activeSavedCount = opts.inline
    ? 0
    : getSavedWindows().filter(
      (saved) =>
        saved.fishId === f.id && BigInt(saved.endEsec) > BigInt(nowEorzea),
    ).length;
  const displayName = `${f.name}${activeSavedCount ? ` (${activeSavedCount})` : ""}`;
  const localWindow = alwaysUp
    ? "always up"
    : `${f.windowStart.split(":").slice(0, 2).join(":")}-${f.windowEnd.split(":").slice(0, 2).join(":")}`;
  const windowDisplay = alwaysUp ? localWindow : `ET ${localWindow}`;
  const note = getNote(f.id);
  const hasNote = note.length > 0;
  div.innerHTML = `
      <input type="checkbox" aria-label="Mark ${escapeHtml(f.name)} as caught" ${opts.caught ? "checked" : ""} />
      <button type="button" class="fav-star${isFaved ? " faved" : ""}" data-fish-id="${f.id}" aria-label="${isFaved ? "Remove fish from favourites" : "Add fish to favourites"}">${isFaved ? "&#9829;" : "&#9825;"}</button>
      <div class="fish-body" tabindex="0" role="button" aria-label="Show schedule for ${escapeHtml(f.name)}">
        <span class="fish-name truncate">${escapeHtml(displayName)}${hasNote ? `<span class="fish-note-indicator" data-fish-id="${f.id}" title="Has note">&#128221;</span>` : ""} ${nextStr}</span>
        <span class="fish-meta truncate"><span class="spot-link" role="button" tabindex="0">${escapeHtml(f.spot.name)}</span> | ${escapeHtml(f.region)} | ${escapeHtml(f.patch)} | ${escapeHtml(weatherStr)}${escapeHtml(uptimeStr)} | ${escapeHtml(windowDisplay)}</span>
        ${requirementsHtml}
      </div>
    `;
  div.querySelector("input").addEventListener("change", () => {
    toggleCaught(f.id);
  });
  div.querySelector(".fav-star").addEventListener("click", (e) => {
    e.stopPropagation();
    toggleFavFish(f.id);
  });
  const fishBody = div.querySelector(".fish-body");
  const selectFish = () => {
    setSelectedFish(f.id);
    const p = readForm();
    if (p.schedule.length === 0)
      p.schedule.push({ startSec: 0, endSec: 86400 });
    saveSearch(p);
    const elStatus = document.getElementById("status");
    elStatus.className = "status";
    query(p, fishBody).catch((err) => {
      elStatus.className = "status error";
      elStatus.textContent = `Error: ${err}`;
    });
  };
  const spotLink = fishBody.querySelector(".spot-link");
  if (spotLink) {
    spotLink.addEventListener("click", (e) => {
      e.stopPropagation();
      showSpotPopup(f.spot, spotLink);
    });
    spotLink.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        e.stopPropagation();
        showSpotPopup(f.spot, spotLink);
      }
    });
  }
  fishBody.addEventListener("click", selectFish);
  fishBody.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectFish();
    }
  });
  return div;
}

let _allFishInfo = [];

function patchSortValue(patch) {
  const parts = String(patch).split(".");
  const major = Number(parts[0]);
  const minor = Number(parts[1] || 0);
  return Number.isFinite(major) && Number.isFinite(minor)
    ? major * 100 + minor
    : Infinity;
}

function compareFishBySort(a, b, sort, nowUnix, nowEorzea) {
  if (sort === "uptime") {
    const uptimeDiff = (a.fishUptime ?? Infinity) - (b.fishUptime ?? Infinity);
    if (uptimeDiff !== 0) return uptimeDiff;
  } else if (sort === "patch") {
    const patchDiff = patchSortValue(b.patch) - patchSortValue(a.patch);
    if (patchDiff !== 0) return patchDiff;
  } else if (sort === "next-window") {
    const aWindow = getNextFishWindow(a.id, nowUnix, nowEorzea);
    const bWindow = getNextFishWindow(b.id, nowUnix, nowEorzea);
    if (aWindow && !bWindow) return -1;
    if (!aWindow && bWindow) return 1;
    if (aWindow && bWindow) {
      const nextDiff =
        BigInt(aWindow.startEsec) < BigInt(bWindow.startEsec)
          ? -1
          : BigInt(aWindow.startEsec) > BigInt(bWindow.startEsec)
            ? 1
            : 0;
      if (nextDiff !== 0) return nextDiff;
    }
  }
  return a.name.localeCompare(b.name);
}

function hydrateFishNextWindows(elements) {
  if (!elements || elements.length === 0) return;
  _fishHydrationGen++;
  const gen = _fishHydrationGen;
  let i = 0;
  const CHUNK = 8;
  function processChunk() {
    if (gen !== _fishHydrationGen) return;
    const nowUnix = Math.floor(Date.now() / 1000);
    const nowEorzea = unix_to_eorzea_esec(BigInt(nowUnix));
    const end = Math.min(i + CHUNK, elements.length);
    for (; i < end; i++) {
      const el = elements[i];
      if (!el.isConnected) continue;
      const fishId = parseInt(el.dataset.fishId, 10);
      const next = getNextFishWindow(fishId, nowUnix, nowEorzea);
      if (next) {
        const startUnix = Number(unix_from_eorzea_time(BigInt(next.startEsec)));
        const endUnix = Number(unix_from_eorzea_time(BigInt(next.endEsec)));
        const { whenStr, state } = fmtWhen(
          startUnix - nowUnix,
          endUnix - nowUnix,
        );
        el.className = `fish-next-window ${state}`;
        el.textContent = state === "future" ? "in " + whenStr : whenStr;
        el.insertAdjacentHTML("afterend", fishEyesIndicator(next));
      } else {
        el.className = "fish-next-window";
        el.textContent = "";
      }
    }
    if (i < elements.length) {
      requestAnimationFrame(processChunk);
    } else {
      _fishHydrationActive = false;
    }
  }
  _fishHydrationActive = true;
  requestAnimationFrame(processChunk);
}

function renderFishList() {
  const caught = getList(CAUGHT_KEY);
  const favs = getList(FAV_KEY);
  const caughtSet = new Set(caught);
  const favSet = new Set(favs);
  const hideCaught = document.getElementById("hide-caught").checked;
  const bigFishOnly = document.getElementById("big-fish-only").checked;
  const q = document
    .getElementById("fish-list-search")
    .value.trim()
    .toLowerCase();
  const sort = document.getElementById("fish-list-sort").value;
  const nowUnix = Math.floor(Date.now() / 1000);
  const nowEorzea = unix_to_eorzea_esec(BigInt(nowUnix));
  const scroll = document.getElementById("fish-list-scroll");

  let filtered = _allFishInfo;
  if (q) {
    const parts = q.split("&&").map((s) => s.trim());
    for (const part of parts) {
      const pq = part.toLowerCase();
      if (pq.startsWith("zone:")) {
        const zoneQ = pq.slice(5);
        filtered = filtered.filter(
          (f) =>
            f.region.toLowerCase().includes(zoneQ) ||
            f.spot.name.toLowerCase().includes(zoneQ),
        );
      } else if (pq.startsWith("patch:")) {
        const patchQ = pq.slice(6);
        filtered = filtered.filter((f) => f.patch.includes(patchQ));
      } else if (pq.startsWith("uptime>") || pq.startsWith("uptime<")) {
        const op = pq[6];
        const raw = part.slice(7);
        const isPct = raw.endsWith("%");
        const val = parseFloat(isPct ? raw.slice(0, -1) : raw);
        if (isNaN(val)) continue;
        const threshold = isPct ? val / 100 : val;
        filtered = filtered.filter((f) =>
          op === ">" ? f.fishUptime > threshold : f.fishUptime < threshold,
        );
      } else {
        filtered = filtered.filter((f) => f.name.toLowerCase().includes(pq));
      }
    }
  }

  if (hideCaught) {
    filtered = filtered.filter((f) => !caughtSet.has(f.id));
  }
  if (bigFishOnly) {
    filtered = filtered.filter((f) => f.bigFish);
  }

  filtered = filtered.slice().sort((a, b) => {
    const aFav = favSet.has(a.id) ? 0 : 1;
    const bFav = favSet.has(b.id) ? 0 : 1;
    if (aFav !== bFav) return aFav - bFav;
    return compareFishBySort(a, b, sort, nowUnix, nowEorzea);
  });

  const total = _allFishInfo.length;
  const caughtCount = caught.length;
  const favCount = favs.length;
  document.getElementById("fish-list-stats-text").textContent =
    `| ${caughtCount}/${total} caught | ${favCount} faved` +
    (q || hideCaught || bigFishOnly ? ` | ${filtered.length} shown` : "") +
    ` | Last updated: ${new Date().toLocaleTimeString()}`;

  scroll.innerHTML = "";
  if (filtered.length === 0) {
    scroll.innerHTML =
      '<div class="fish-empty">No fish match your filter.</div>';
    return;
  }

  const frag = document.createDocumentFragment();
  const deferredSpans = [];
  for (const f of filtered) {
    const opts = {
      caught: caughtSet.has(f.id),
      faved: favSet.has(f.id),
      deferNextWindow: true,
    };
    const card = createFishCard(f, opts);
    const span = card.querySelector(".fish-next-deferred");
    if (span) deferredSpans.push(span);
    frag.appendChild(card);
  }
  scroll.appendChild(frag);
  hydrateFishNextWindows(deferredSpans);
}

let _fishSearchTimer = null;
let _fishFilterTimer = null;

function debouncedRenderFishList() {
  clearTimeout(_fishFilterTimer);
  _fishFilterTimer = setTimeout(renderFishList, 80);
}

document.getElementById("fish-list-search").addEventListener("input", () => {
  clearTimeout(_fishSearchTimer);
  _fishSearchTimer = setTimeout(renderFishList, 100);
});

document.getElementById("hide-caught").addEventListener("change", () => {
  debouncedRenderFishList();
});

document.getElementById("big-fish-only").addEventListener("change", () => {
  debouncedRenderFishList();
});

document.getElementById("fish-list-sort").addEventListener("change", () => {
  debouncedRenderFishList();
});

document
  .getElementById("fish-list-search-clear")
  .addEventListener("click", () => {
    document.getElementById("fish-list-search").value = "";
    renderFishList();
  });

document.getElementById("filter-help").addEventListener("click", (event) => {
  const box = document.getElementById("filter-help-box");
  const visible = box.classList.toggle("visible");
  event.currentTarget.setAttribute("aria-expanded", String(visible));
});

document.addEventListener("click", (e) => {
  if (
    !e.target.closest("#filter-help") &&
    !e.target.closest("#filter-help-box")
  ) {
    document.getElementById("filter-help-box").classList.remove("visible");
    document
      .getElementById("filter-help")
      .setAttribute("aria-expanded", "false");
  }
  if (_spotPopup && !e.target.closest(".spot-popup") && !e.target.closest(".spot-link")) {
    closeSpotPopup();
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && _spotPopup) {
    closeSpotPopup();
  }
});

initializeNotificationControls();
main();
