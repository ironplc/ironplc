=================
Enabling Features
=================

IronPLC aims to let you take code from another PLC environment and use it
without changes. However, the IEC 61131-3 standard has evolved through
multiple editions, and some features require you to tell IronPLC which
edition your code targets.

---------------------------------
Editions of the Standard
---------------------------------

The IEC 61131-3 standard has been published in several editions:

**Edition 2 (2003)**
   The widely deployed baseline. IronPLC uses this edition by default.

**Edition 3 (2013)**
   Adds new data types (:doc:`LTIME </reference/language/data-types/ltime>`,
   :doc:`LDATE </reference/language/data-types/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/ltime-of-day>`,
   :doc:`LDATE_AND_TIME </reference/language/data-types/ldate-and-time>`)
   and other language enhancements.

Editions are additive — enabling a later edition includes all features from
earlier editions.

See :doc:`/reference/language/edition-support` for a complete list of
features that require a specific edition.

---------------------------------
How to Enable an Edition
---------------------------------

Command Line
^^^^^^^^^^^^

Pass the ``--std-iec-61131-3`` flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --std-iec-61131-3=2013 main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.

Visual Studio Code
^^^^^^^^^^^^^^^^^^

Set the :code:`ironplc.std61131Version` setting to :code:`2013`:

1. Open :menuselection:`File --> Preferences --> Settings`
   (or :menuselection:`Code --> Preferences --> Settings` on macOS).
2. Search for ``ironplc``.
3. Change :guilabel:`Std 61131 Version` to ``2013``.

Or add it directly to your :file:`settings.json`:

.. code-block:: json

   {
     "ironplc.std61131Version": "2013"
   }

See :doc:`/reference/editor/settings` for all extension settings.

---------------------------------
Future Capabilities
---------------------------------

As IronPLC adds support for more of the standard and its revisions, new
capability flags may be introduced. This page will be updated to cover
each new flag as it becomes available.
