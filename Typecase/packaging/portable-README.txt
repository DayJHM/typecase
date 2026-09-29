Typecase VERSION_HERE - portable client (Windows 10 / Windows 11, 64-bit)
=========================================================================

Typecase is a local-first Windows font manager: it shows the fonts this PC
already has, previews and caches families from Google Fonts, installs and
uninstalls them for your user account, and backs families up as ZIP exports.

This is the PORTABLE build: there is no installer. Run Typecase-portable.exe
directly - from a USB stick, a network share, a downloads folder. Nothing is
added to the Start menu and the app writes no registry entries of its own.

REQUIREMENTS
  * Windows 10 version 1809 or later, or Windows 11 (64-bit)
  * Microsoft Edge WebView2 runtime, which Typecase uses to draw its interface.
    Windows 11 and Windows 10 (April 2018 or later) ship it as part of the
    operating system, so it is normally already present. If it is missing,
    Typecase says so on startup; you can install it from:
      WEBVIEW2_URL_HERE

YOUR DATA
  Typecase keeps fonts, library state and exports under:

    %LOCALAPPDATA%\Typecase\
      fonts\      downloaded families, and copies you kept of external fonts
      state\      library.json - what is cached and what is installed
      catalog\    catalog.json - present after you run a catalog refresh
      exports\    export ZIPs

  That location is shared with the installed build on purpose (CONTEXT.md
  section 35: a portable executable does not imply portable data). Keeping data
  beside the exe is a separate, later feature and is not offered here.

INSTALLING FONTS
  Installing a family writes to your own user account only, with no
  administrator prompt. "Everyone on this PC" asks for elevation once, for that
  single operation - Typecase itself never runs elevated.

REMOVING IT
  Delete this executable. Your data under %LOCALAPPDATA%\Typecase\ is
  deliberately left alone; delete that folder yourself if you want it gone.

VERIFY THE DOWNLOAD
  SHA-256 of Typecase-portable.exe:
    HASH_HERE

Built from https://github.com/DayJHM/typecase - REV_HERE
