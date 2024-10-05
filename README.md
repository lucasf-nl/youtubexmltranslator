# YouTube XML RSS translator

YouTube provides an endpoint returning an XML file for each channel that contains
the 22 most recent videos. This file has a custom schema that is not compatible with RSS feed readers.
To solve this problem this web server converts the XML file into an RSS feed on the fly.
