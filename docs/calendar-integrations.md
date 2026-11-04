# Calendar integrations

Demerzel can export an iCalendar (`.ics`) file, write activity directly to Apple Calendar on macOS, or sync events to Google Calendar. Completed process sessions and AI agent fixes become calendar events. Agent events include token totals when the selected runner provides usage data. Stable event identifiers let later updates replace the existing event instead of duplicating it.

## Google Calendar setup

This open-source project does not ship a shared OAuth credential. Each user supplies a Google OAuth Client ID from their own Google Cloud project:

1. Create or select a project in [Google Cloud Console](https://console.cloud.google.com/).
2. Enable the Google Calendar API for that project.
3. Configure the OAuth consent screen and add your Google account as a test user if the app is in testing mode.
4. Create an OAuth client with application type **Desktop app**. Do not create or share a client secret; installed desktop apps cannot keep one confidential.
5. Copy the Client ID into **Settings → Calendar → Google OAuth Client ID** in Demerzel and choose **Connect Google Calendar**.
6. Approve Calendar event access in the browser. Demerzel returns to the app through a temporary loopback callback on `127.0.0.1`.

Demerzel uses OAuth authorization code with PKCE. The refresh token is saved in the operating system credential store (Keychain on macOS, Credential Manager on Windows, and Secret Service on supported Linux desktops), not in the settings file or repository. Disconnecting revokes the token and removes it from the credential store.

Google may require OAuth app verification before accounts outside the configured test users can connect. Google can also show an unverified-app warning while an app is in testing or has not completed verification.

## Apple Calendar setup

On macOS, enable **Sync with Apple Calendar**, load the available calendars, and select a destination. macOS asks for Calendar automation permission the first time Demerzel writes an event. Events are upserted using a private Demerzel marker in their description.

Apple Calendar integration is only available on macOS. On other platforms, use Google Calendar or `.ics` export.

## iCalendar export

Choose a `.ics` destination to maintain an export that can be imported into other calendar applications. Importing an `.ics` file into Google Calendar is a snapshot; it does not create a live subscription. For ongoing sync, connect Google Calendar directly.
