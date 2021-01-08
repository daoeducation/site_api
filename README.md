= A multi-signed bitcoin based Service for maintaining timestamped Bulletins.

Service:
  The Service builds, stores, and timestamps Bulletins.

Entries:
  - A Entry is any payload that may be timestamped (a file).
  
  - It's referenced in a Bulletin, thus timestamped.

  - A Entry may be uploaded anonymously.

  - Entries uploaded by the service are special, they can be assumed to be signed by the service itself.

  - Entries can be encrypted for other entities.

  - A Entry may or may not be signed.

  - The Service may demand Entries to be signed before adding them.

  - It's contents may be encrypted for another Entity. (pgp)
    The Service decides which documents to incorporate and if they require signature.
    Own Entries are produced by the service itself don't need to be signed.
    External Entries are uploaded by Signers|Entities and can be encrypted for other entities.

  - It's contents may be signed by an Entity.

Individual:
  - An entity is an individual person or machine.
  - It can sign and timestamp documents.
  - It has several Signatures

Group:
  - A Group is founded by Entities.
  - It can sign and timestamp documents.
  - It determines a governance model and multi-signature requirements.

Signature Certificate:
  - A Signature Certificate links an Entity with a signing method.
  - It can be more or less reliable depending on the signing method.
  - Signatures can be used to sign Entries or to establish web-of-trust between signature methods.

Bulletin:
  - Bulletins have a timestamp and contain Entries (their hashes). A Bulletin is referenced on a Bitcoin transactions OP_RETURN.

= Signature methods, strength and weakness:

- Video: A person signs a document or another key by reading it out loud on video. Or at least reading the relevant parts.
  - Proves the person actually read the Entry. 
  - It's easy to produce by them, but takes time.
  - If they don't read the whole document, they can argue they didn't accept all clauses.
  - If they just read the document fingerprint they should also state on video that they
    calculated the hash from the document they read and are accepting.
    We can provide tools for that.
  - Do we also request their national ID and proof?

  - Repudiation:
    - A deep fake or other video manipulation happened.
    - If the signer just reads a fingerprint, they can claim it was all provided to them and they didn't calculate the fingerprint themselves.

- Email / Whatsapp / Telegram: A person signs a document by attaching it on a message stating their agreement to the terms.
  - Proves someone with the credentials to that messaging app read the document.
  - It's the simplest to produce.
  - It's very weak.
  - Changing the channel, with things like magic links, requires a handshake or revalidation.
  
  - Repudiation:
    - "That's not my email address"
    - "My email or email provider got hacked."
    - "My email does not have SPF so anyone can send emails pretending to be me."

- PGP: A person signs documents with an asymmetric key.
  - Proves the person with the private key read the document.
  - It's easy to produce and can be produced via API.
  - It's strong, but depends on the certification method.

= Protocol Actions reflected as Entries:
  - Add Entity:
    Declare myself to be an entity in posession of a Signature.
    Depending on the Signature it can be:
      - The video itself + metadata.

  - Request web of trust:
    - Use an existing signature method to onboard a new method.
    - Referente the other method.

  - Reply for web of trust:
    - Use the new method to accept the old method's request.

  - Invite to Group: A number of Users can act as a group.
    A group sets governance rules for passing votes. N-of-M votes.

  - Accept group invitation.

  - Change Group governance rules:
    Governance rules tell us how many n-of-m are needed.
    This message needs to be signed by the group.

  - Message:
    - A new message may have been sent on a thread.
    - We don't include the message but fingerprint it.
    - It may reference a thread or previous message.


# Workflows:

## Onboarding + Test:

  - El cliente deja su email en la web.

  - Le escribe ace@constata.eu:
    Hola, soy Ace, tu Asistente de Constatación Electrónica.

    Con mi ayuda podrás darle seguridad a toda tu actividad comercial en internet.
    
    Por ejemplo, si me pones en copia en un email, le hago un sello de tiempo tanto al correo como
    a los archivos adjuntos. Así podemos probar que un documento o conversación existía al momento de ser sellado,
    y no fue creado luego.
    Mis sellos de tiempo quedan en la blockchain de Bitcoin, son virtualmente indestructibles, y tienen validez legal.
    También me guardo el contenido de la conversación de forma segura por el tiempo que haga falta.

    Si una conversación es acerca de un contrato que los participantes deben firmar, puedo encargarme
    de hablar con cada uno de ellos para que lo firmen digitalmente de la forma que mejor les acomode.
    Solo tienes que pedírmelo así: "Ani, encárgate de las firmas".
    La fecha de las firmas también queda registrada en la blockchain de Bitcoin y tiene validez legal.
    
    Hagamos una prueba, envía un archivo cualquiera a prueba@constata.eu, y ponme en copia, yo soy: ace@constata.eu

  - El cliente escribe:
    subject: Probando
    to: prueba@constata.eu
    cc: ace@constata.eu

    Adjunta Documento.PDF

  - ace@constata.eu
    Recibí tu correo con el adjunto "Documento.PDF", la huella digital es "AOEUAEUOEUOEUA".
    Puedes verlo en mi boletín "https://constata.eu/documents/AOEUAOEUAAOEUA"
    Pronto se aplicará el sello de tiempo en Blockchain.

    El documento fue entregado a prueba@constata.eu.
    Si quieres también me encargo de que se firme digitalmente.
    Haz click aquí para decirme que documentos y quienes deberían firmar.
    https://contsata.eu/documents/121212112/require_signature

  - El cliente hace click, se le muestra una página con:
    - Nombre - Email de los destinatarios.
    - Listado de cosas a firmar:
      - Un extracto completo de la conversación hasta el momento.
      - Los archivos adjuntos.

    El cliente selecciona personas y archivos, y da OK.


  - ace@constata.eu le escribe a cada parte para pedirle que firme,
    este mensaje contiene el link específico para cada persona.

  - Revisamos si la persona ya tiene una clave.
    Como todavía no tiene, se hace el onboarding de firma electrónica.
    - Opcionalmente, firma con yubikey, trezor, etc.

  - Vamos a crearte una firma digital fuerte.
    A diferencia de otras firmas electrónicas y digitales, esta firma se generará ahora mismo en tu ordenador,
    y solo tu vas a conocerla. 
    Tu firma electrónica es algo muy personal, ni siquiera nosotros vamos a guardarla en la nube,
    por eso te enseñaremos a hacer backup tu mismo para que nunca la pierdas.
    Si eres muy consciente de la seguridad, puedes apagar tu conexión a internet ahora.

  - Haz un autógrafo o escribe tu nombre en este recuadro.
    - Si no quieres firmar, puedes dibujar alguna otra cosa, lo que quieras.
    - Esto no es una firma holográfica, solo que vamos a usar los trazos como uno de los ingredientes para 
      tu firma digital única e irreproducible.

  - Ponle una contraseña a tu firma, no hay requisitos, pero recomendamos que sea una frase fácil de recordar.

  - Ahora, como prometimos, vamos a hacer backup de tu firma.
    La contraseña que ingresaste no se incluye en el backup, eso debes recordarlo tu.

  - 
  
  - Genial! Tu firma digital fue creada.
    
    Worflow interno:
      - Les ofrece un menú de firma:
        - Ya tienes un certificado de firma electrónica de otro proveedor.
        - Creamos uno rápidamente.
          
    - Les pregunta si ya tienen una firma electrónica externa (y les deja usarla)
    - 
    La persona accede al link donde ve:
      - Una representación como en webmail del mensaje.
      - Los adjuntos.
      - Las instrucciones para verificar independientemente el documento que se está firmando.


  - ace@constata.eu
    Tu documento ya fue sellado de tiempo en blockchain. Adjunto el boletín correspondiente para que puedas verificarlo tu mismo.

  - 


##
  


  

  
   



= Verification:
  - All Entries can be verified on-chain.
  - We provide a search tool to drop-in a document (or paste its hash) and find out:
    - Its timestamp.
    - Verify its Signature.
  - We produce 'detached' signatures so file hashes don't change.
  - A document may have been signed but not timestamped.
  - A person with a Entry 

= Design principles:

- No users and using public keys instead make the service more secure and reliable:
  - All actions are signed with a key unknown to the Service.
  - The service cannot be hacked to take actions on behalf of Entities.
  - Password recoveries are self-managed. Entities may backup their keys.

- No merkle trees. We *can* and *want to* hit a roadblock due to extreme usage before optimizing that.
  Merkle trees are harder to explain.

- The system does not rely on TLS and client certs for Entities, that way it can decouple Entity keys from sockets. This makes it easier to use hardware security modules and crypto wallets, or air-gapped systems.
  Nevertheless, all communications use TLS and may use client certs, which when compromised still don't affect Entity signatures.

- Clients act similar to bitcoin wallets in that they need to source a private key.

- The main data set is built from Entries, but other views on that data may be built, for example, to check on entities history, validity or expiration of signature certificates, etc.

= Value added by a centralized service:

Signing and self-certification are technically possible, but technology is not widespread enough. 
All attempts to make these practices mainstream require a centralized service to 'fill-in' the gaps in the user experience.

- Entities don't need to have Bitcoin in order to timestamp documents. (like in opentimestamps)
- Batching documents in Bulletins reduces costs. (like in opentimestamps)
- A business model for timestamping makes the service more reliable (vs opentimestamps community driven calendar servers).
- Storing the timestamped documents is convenient for customers who may not be able to reliably store them themselves.
- Being naive constructing bulletins and storing all the metadata makes it easier to construct a natural language explanation that may be understood by judges directly, without calling in a forensic expert.
- We can give human monitored KYC for entities, which would make their signatures legally binding. In those terms, we can issue 'Signature Certificates' as a type of Entry.
- The PGP world relies on key storing services like the MIT one, which can't keep up with traffic. We can build a similar service based of Entries.
- While we would never store secret keys, we may make it easy to backup secret keys.
- Multi-sig setups require state to be shared. We can keep that state. (partial signatures).

