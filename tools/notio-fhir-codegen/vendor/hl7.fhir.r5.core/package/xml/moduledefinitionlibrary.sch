<?xml version="1.0" encoding="UTF-8"?>
<sch:schema xmlns:sch="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt2">
  <sch:ns prefix="f" uri="http://hl7.org/fhir"/>
  <sch:ns prefix="h" uri="http://www.w3.org/1999/xhtml"/>
  <!-- 
    This file contains just the constraints for the profile ShareableLibrary
    It includes the base constraints for the resource as well.
    Because of the way that schematrons and containment work, 
    you may need to use this schematron fragment to build a, 
    single schematron that validates contained resources (if you have any) 
  -->
  <sch:pattern>
    <sch:title>f:Library</sch:title>
    <sch:rule context="f:Library">
      <sch:assert test="count(f:extension[@url = 'http://hl7.org/fhir/StructureDefinition/cqf-inputParameters']) &lt;= 1">extension with URL = 'http://hl7.org/fhir/StructureDefinition/cqf-inputParameters': maximum cardinality of 'extension' is 1</sch:assert>
      <sch:assert test="count(f:content) &lt;= 0">content: maximum cardinality of 'content' is 0</sch:assert>
    </sch:rule>
  </sch:pattern>
  <sch:pattern>
    <sch:title>f:Library/f:relatedArtifact</sch:title>
    <sch:rule context="f:Library/f:relatedArtifact">
      <sch:assert test="count(f:id) &lt;= 1">id: maximum cardinality of 'id' is 1</sch:assert>
      <sch:assert test="count(f:type) &gt;= 1">type: minimum cardinality of 'type' is 1</sch:assert>
      <sch:assert test="count(f:type) &lt;= 1">type: maximum cardinality of 'type' is 1</sch:assert>
      <sch:assert test="count(f:label) &lt;= 1">label: maximum cardinality of 'label' is 1</sch:assert>
      <sch:assert test="count(f:display) &lt;= 1">display: maximum cardinality of 'display' is 1</sch:assert>
      <sch:assert test="count(f:citation) &lt;= 1">citation: maximum cardinality of 'citation' is 1</sch:assert>
      <sch:assert test="count(f:document) &lt;= 1">document: maximum cardinality of 'document' is 1</sch:assert>
      <sch:assert test="count(f:resource) &gt;= 1">resource: minimum cardinality of 'resource' is 1</sch:assert>
      <sch:assert test="count(f:resource) &lt;= 1">resource: maximum cardinality of 'resource' is 1</sch:assert>
      <sch:assert test="count(f:resourceReference) &lt;= 1">resourceReference: maximum cardinality of 'resourceReference' is 1</sch:assert>
      <sch:assert test="count(f:publicationStatus) &lt;= 1">publicationStatus: maximum cardinality of 'publicationStatus' is 1</sch:assert>
      <sch:assert test="count(f:publicationDate) &lt;= 1">publicationDate: maximum cardinality of 'publicationDate' is 1</sch:assert>
    </sch:rule>
  </sch:pattern>
</sch:schema>
