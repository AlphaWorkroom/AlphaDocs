# Local signing in AlphaDocs

Open **Tools → eSignature**. Enter your name and each signer’s name, choose the fields they need, then create an `.asign` file. The request contains a frozen copy of every document tab. Your original document stays editable; later edits do not change an existing request.

Share the request file using your usual file-transfer method. The recipient opens **Tools → eSignature → Open signing file**, chooses their name, reviews the document and fills their assigned fields. Signatures and initials can be typed or drawn. Date signed is filled from the recipient’s device clock when they review their response. They confirm their intent, review the completed fields, and save an `.asign-response` file to return to you. Decline uses the same response-file process.

Open your original request and choose **Import returned response**. AlphaDocs checks its request ID, document hash, assignment-manifest hash, signer and field ownership. Conflicting responses are rejected; an identical response can be imported again safely. Save the updated request to retain its status and responses. When everyone has signed, **Export completed PDF** includes the frozen document, a separate signing section and a local audit record.

Signatures currently appear in the appended signing section, rather than fields positioned over arbitrary document content. Delivery is file-based; AlphaDocs does not send email, host signing pages or automatically invite nearby devices for signing. Live document collaboration is a separate feature.

Names are self-declared and timestamps come from local device clocks. These files do not authenticate a person or provide certificate-backed digital signatures or trusted server timestamps. Hash checks bind a response to the request you already hold; they do not establish the sender’s identity. Keep your original request and returned responses.
