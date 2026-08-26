# Constitution

Rules every change inherits. `propose` and `apply` both read this file first.

**Write this by hand.** Delta ships no command that generates it by reading
your repository, and that omission is deliberate: auto-generated context files
measurably reduce agent success and raise cost, while hand-written ones improve
both. Ten lines you wrote beat sixty a tool inferred.

**Litmus test for every line below: would removing this line cause an agent to
make a mistake it would not otherwise make?** If not, delete the line. This
file is not a style guide, not an onboarding document, and not a place for
things a competent contributor already does. Keep it under 60 lines. When it
grows past that, something in it has stopped being non-negotiable.

Replace everything below with your own rules.

## Layering

<!-- Which direction may dependencies point, and what may never import what.
     Example:
     - The domain layer imports nothing from web, persistence, or messaging.
     - Controllers call services. Controllers never touch repositories.       -->

- ...

## Never touch

<!-- Code, data, or config that must not change as a side effect of any
     change, and what to do instead when a change appears to require it.
     Example:
     - Never edit a file under db/migrations/ that has shipped. Add a new one.
     - Never change the wire format of events on the settlement topic.        -->

- ...

## Errors

<!-- What is thrown or returned, what is caught and where, what surfaces to a
     caller, and what must never be swallowed.
     Example:
     - No empty catch blocks. Rethrow as a domain error or handle it.
     - Errors crossing the API boundary carry a code, never a stack trace.    -->

- ...

## Logging

<!-- Levels, required context, and what may never be logged.
     Example:
     - Never log card numbers, tokens, or full request bodies.
     - Every log line at the service boundary carries the correlation id.     -->

- ...
