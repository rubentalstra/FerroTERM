<?xml version="1.0" encoding="UTF-8"?>
<sch:schema xmlns:sch="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt2">
  <sch:ns prefix="f" uri="http://hl7.org/fhir"/>
  <sch:ns prefix="h" uri="http://www.w3.org/1999/xhtml"/>
  <!-- 
    This file contains just the constraints for the profile ElementDefinition
    It includes the base constraints for the resource as well.
    Because of the way that schematrons and containment work, 
    you may need to use this schematron fragment to build a, 
    single schematron that validates contained resources (if you have any) 
  -->
  <sch:pattern>
    <sch:title>f:ElementDefinition</sch:title>
    <sch:rule context="f:ElementDefinition">
      <sch:assert test="count(f:extension[@url = 'http://hl7.org/fhir/StructureDefinition/elementdefinition-allowedUnits']) &lt;= 1">extension with URL = 'http://hl7.org/fhir/StructureDefinition/elementdefinition-allowedUnits': maximum cardinality of 'extension' is 1</sch:assert>
      <sch:assert test="count(f:representation) &lt;= 0">representation: maximum cardinality of 'representation' is 0</sch:assert>
      <sch:assert test="count(f:slicing) &lt;= 0">slicing: maximum cardinality of 'slicing' is 0</sch:assert>
      <sch:assert test="count(f:short) &lt;= 0">short: maximum cardinality of 'short' is 0</sch:assert>
      <sch:assert test="count(f:contentReference) &lt;= 0">contentReference: maximum cardinality of 'contentReference' is 0</sch:assert>
      <sch:assert test="count(f:fixed[x]) &lt;= 0">fixed[x]: maximum cardinality of 'fixed[x]' is 0</sch:assert>
      <sch:assert test="count(f:pattern[x]) &lt;= 0">pattern[x]: maximum cardinality of 'pattern[x]' is 0</sch:assert>
      <sch:assert test="count(f:isModifier) &lt;= 0">isModifier: maximum cardinality of 'isModifier' is 0</sch:assert>
      <sch:assert test="count(f:isSummary) &lt;= 0">isSummary: maximum cardinality of 'isSummary' is 0</sch:assert>
    </sch:rule>
  </sch:pattern>
  <sch:pattern>
    <sch:title>f:ElementDefinition/f:type</sch:title>
    <sch:rule context="f:ElementDefinition/f:type">
      <sch:assert test="count(f:profile) &lt;= 0">profile: maximum cardinality of 'profile' is 0</sch:assert>
      <sch:assert test="count(f:aggregation) &lt;= 0">aggregation: maximum cardinality of 'aggregation' is 0</sch:assert>
    </sch:rule>
  </sch:pattern>
</sch:schema>
